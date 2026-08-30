use serde_json::Value;

use crate::settings::Settings;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                  AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// 把文本翻译成目标语言。需要联网。
pub fn translate(text: &str, target: &str, s: &Settings) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(String::new());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    match s.translate_provider.as_str() {
        "mymemory" => mymemory(&client, text, target),
        "deepl" => deepl(&client, text, target, &s.translate_key),
        "custom" => custom(
            &client,
            text,
            target,
            &s.translate_endpoint,
            &s.translate_key,
            &s.translate_model,
        ),
        _ => google(&client, text, target),
    }
}

/* ------------------------------ Google 免费端点 ------------------------------ */
fn google(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&q={}",
        urlencode(target),
        urlencode(text)
    );
    let v: Value = client
        .get(url)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?
        .json()
        .map_err(|e| format!("解析响应失败: {e}"))?;

    let mut out = String::new();
    if let Some(rows) = v.get(0).and_then(|x| x.as_array()) {
        for row in rows {
            if let Some(seg) = row.get(0).and_then(|x| x.as_str()) {
                out.push_str(seg);
            }
        }
    }
    if out.is_empty() {
        Err("翻译服务没有返回结果（可能是网络不可用）".into())
    } else {
        Ok(out)
    }
}

/* -------------------------------- MyMemory -------------------------------- */
fn mymemory(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}",
        urlencode(text),
        urlencode(&format!("{}|{}", "Auto-Detect", normalize(target)))
    );
    let v: Value = client
        .get(url)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?
        .json()
        .map_err(|e| format!("解析响应失败: {e}"))?;

    match v
        .get("responseData")
        .and_then(|d| d.get("translatedText"))
        .and_then(|t| t.as_str())
    {
        Some(t) if !t.is_empty() => Ok(t.to_string()),
        _ => {
            let detail = v
                .get("responseDetails")
                .and_then(|d| d.as_str())
                .unwrap_or("未知错误");
            Err(format!("MyMemory 翻译失败：{detail}"))
        }
    }
}

/* ---------------------------------- DeepL ---------------------------------- */
fn deepl(client: &reqwest::blocking::Client, text: &str, target: &str, key: &str) -> Result<String, String> {
    if key.is_empty() {
        return Err("DeepL 需要填写 API Key".into());
    }
    let base = if key.ends_with(":fx") {
        "https://api-free.deepl.com"
    } else {
        "https://api.deepl.com"
    };
    let resp = client
        .post(format!("{base}/v2/translate"))
        .header("Authorization", format!("DeepL-Auth-Key {key}"))
        .form(&[
            ("text", text.to_string()),
            ("target_lang", deepl_lang(target)),
        ])
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let v: Value = resp.json().map_err(|e| format!("解析响应失败: {e}"))?;
    v.get("translations")
        .and_then(|t| t.get(0))
        .and_then(|t| t.get("text"))
        .and_then(|t| t.as_str())
        .map(|t| t.to_string())
        .ok_or_else(|| "DeepL 返回异常".to_string())
}

/* ------------------- 自定义 OpenAI 兼容接口（可选 AI 翻译） ------------------- */
fn custom(
    client: &reqwest::blocking::Client,
    text: &str,
    target: &str,
    endpoint: &str,
    key: &str,
    model: &str,
) -> Result<String, String> {
    if endpoint.is_empty() {
        return Err("请填写自定义接口地址".into());
    }
    let model = if model.is_empty() { "gpt-4o-mini" } else { model };
    let prompt = format!(
        "把下面的内容翻译成 {}。只输出译文，不要任何解释、不要保留 Markdown 代码块标记：\n\n{}",
        lang_name(target),
        text
    );
    let body = serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": "You are a professional translator."},
            {"role": "user", "content": prompt}
        ]
    });

    let mut req = client.post(endpoint).json(&body);
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }
    let v: Value = req
        .send()
        .map_err(|e| format!("请求失败: {e}"))?
        .json()
        .map_err(|e| format!("解析响应失败: {e}"))?;

    v.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|t| t.trim().to_string())
        .ok_or_else(|| format!("接口返回异常: {v}"))
}

/* --------------------------------- 工具 --------------------------------- */
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn normalize(t: &str) -> &str {
    match t {
        "zh-CN" | "zh" => "zh-CN",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        other => other,
    }
}

fn deepl_lang(t: &str) -> String {
    match t {
        "zh-CN" | "zh" => "ZH",
        "en" => "EN-US",
        "ja" => "JA",
        "ko" => "KO",
        "fr" => "FR",
        "de" => "DE",
        "es" => "ES",
        other => other,
    }
    .to_uppercase()
}

fn lang_name(t: &str) -> &'static str {
    match t {
        "zh-CN" | "zh" => "简体中文",
        "en" => "English",
        "ja" => "日语",
        "ko" => "韩语",
        "fr" => "法语",
        "de" => "德语",
        "es" => "西班牙语",
        _ => "目标语言",
    }
}
