use color_eyre::eyre::{Context, OptionExt, Result, bail, eyre};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StringEncoding {
    Utf8,
    Windows1252,
}

#[derive(Debug)]
pub struct Header {
    pub ofxheader: u32,
    pub version: u32,
    pub encoding: StringEncoding,
}

pub async fn read_sgml_header(src: &mut BufReader<File>) -> Result<Header> {
    let mut line_buf = Vec::with_capacity(32);

    let mut ofxheader = None;
    let mut data = false;
    let mut version = None;
    let mut security = false;
    let mut encoding = false;
    let mut charset = None;
    let mut compression = false;
    let mut oldfileuid = false;
    let mut newfileuid = false;

    loop {
        line_buf.clear();
        let _ = src.read_until(b'\n', &mut line_buf).await?;

        // Remove newlines
        let header = line_buf.as_slice().trim_ascii();
        if header.is_empty() {
            // Double newline means end of header
            break;
        }

        let mut parts = header.split(|v| *v == b':');
        let key = parts
            .next()
            .expect("Non zero length line should have at least one part");
        let value = parts
            .next()
            .ok_or_else(|| eyre!("Header line missing colon: {:?}", &line_buf))?;

        match key {
            b"OFXHEADER" => {
                let parsed = str::from_utf8(value)
                    .wrap_err("invalid utf8 in OFXHEADER")
                    .and_then(|v| v.parse::<u32>().wrap_err("Failed to parse OFXHEADER"))?;
                if ofxheader.replace(parsed).is_some() {
                    bail!("Repeated header 'OFXHEADER")
                }
            }
            b"DATA" => {
                if data {
                    bail!("Repeated header 'DATA");
                }
                match value {
                    b"OFXSGML" => data = true,
                    v => bail!("Unrecognized DATA value: {:?}", v),
                }
            }
            b"VERSION" => {
                let parsed = str::from_utf8(value)
                    .wrap_err("invalid utf8 in VERSION")
                    .and_then(|v| v.parse::<u32>().wrap_err("Failed to parse VERSION"))?;
                if version.replace(parsed).is_some() {
                    bail!("Repeated header 'VERSION")
                }
            }
            b"SECURITY" => {
                if security {
                    bail!("Repeated header 'SECURITY");
                }
                match value {
                    b"NONE" => security = true,
                    v => bail!("Unrecognized SECURITY value: {:?}", v),
                }
            }
            b"ENCODING" => {
                if encoding {
                    bail!("Repeated header 'ENCODING");
                }
                match value {
                    b"USASCII" => encoding = true,
                    v => bail!("Unrecognized ENCODING value: {:?}", v),
                };
            }
            b"CHARSET" => {
                let parsed = match value {
                    b"1252" => StringEncoding::Windows1252,
                    v => bail!("Unrecognized CHARSET value: {:?}", v),
                };
                if charset.replace(parsed).is_some() {
                    bail!("Repeated header 'CHARSET")
                }
            }
            b"COMPRESSION" => {
                if compression {
                    bail!("Repeated header 'COMPRESSION");
                }
                match value {
                    b"NONE" => compression = true,
                    v => bail!("Unrecognized COMPRESSION value: {:?}", v),
                }
            }
            b"OLDFILEUID" => {
                if oldfileuid {
                    bail!("Repeated header 'OLDFILEUID");
                }
                match value {
                    b"NONE" => oldfileuid = true,
                    v => bail!("Unrecognized OLDFILEUID value: {:?}", v),
                }
            }
            b"NEWFILEUID" => {
                if newfileuid {
                    bail!("Repeated header 'NEWFILEUID");
                }
                match value {
                    b"NONE" => newfileuid = true,
                    v => bail!("Unrecognized NEWFILEUID value: {:?}", v),
                }
            }
            h => bail!("Unrecognized header: {:?}", h),
        }
    }

    if !data {
        bail!("Header 'DATA' missing");
    }
    if !security {
        bail!("Header 'SECURITY' missing");
    }
    if !encoding {
        bail!("Header 'ENCODING' missing");
    }
    if !compression {
        bail!("Header 'COMPRESSION' missing");
    }
    if !oldfileuid {
        bail!("Header 'OLDFILEUID' missing");
    }
    if !newfileuid {
        bail!("Header 'NEWFILEUID' missing");
    }

    Ok(Header {
        ofxheader: ofxheader.ok_or_eyre("Header 'OFXHEADER' missing")?,
        version: version.ok_or_eyre("Header 'VERSION' missing")?,
        encoding: charset.ok_or_eyre("Header 'CHARSET' missing")?,
    })
}

pub async fn read_xml_header(src: &mut BufReader<File>) -> Result<Header> {
    let mut line_buf = Vec::with_capacity(128);

    let mut encoding = None;
    let mut ofxheader = None;
    let mut version = None;
    let mut security = false;
    let mut oldfileuid = false;
    let mut newfileuid = false;

    // XML header line
    let _ = src.read_until(b'\n', &mut line_buf).await?;
    {
        let kv_pairs = line_buf
            .trim_ascii()
            .strip_prefix(b"<?xml")
            .ok_or_else(|| eyre!("Missing XML header start in line: {:?}", line_buf))?
            .strip_suffix(b"?>")
            .ok_or_else(|| eyre!("Missing XML header end in line: {:?}", line_buf))?
            .split(|v| *v == b' ');

        for kv_pair in kv_pairs {
            if kv_pair.is_empty() {
                continue;
            }

            let mut kv_parts = kv_pair.split(|v| *v == b'=');
            let key = kv_parts.next().ok_or_eyre("Missing key")?;
            let value = kv_parts
                .next()
                .ok_or_eyre("Missing value")?
                .strip_prefix(b"\"")
                .ok_or_eyre("Value missing opening quote")?
                .strip_suffix(b"\"")
                .ok_or_eyre("Value missing close quote")?;

            if kv_parts.next().is_some() {
                bail!("Unexpected data after key value pair");
            }

            match key {
                b"version" => match value {
                    b"1.0" => {}
                    v => bail!("Unsupported XML version: {:?}", v),
                },
                b"encoding" => match value {
                    b"utf-8" => encoding = Some(StringEncoding::Utf8),
                    v => bail!("Unsupported XML encoding: {:?}", v),
                },
                v => bail!("Unsupported XML header key: {:?}", v),
            }
        }
    }

    // OFX header line
    line_buf.clear();
    let _ = src.read_until(b'\n', &mut line_buf).await?;

    let kv_pairs = line_buf
        .trim_ascii()
        .strip_prefix(b"<?OFX")
        .ok_or_else(|| eyre!("Missing OFX header start in line: {:?}", line_buf))?
        .strip_suffix(b"?>")
        .ok_or_else(|| eyre!("Missing OFX header end in line: {:?}", line_buf))?
        .split(|v| *v == b' ');

    for kv_pair in kv_pairs {
        if kv_pair.is_empty() {
            continue;
        }

        let mut kv_parts = kv_pair.split(|v| *v == b'=');
        let key = kv_parts.next().ok_or_eyre("Missing key")?;
        let value = kv_parts
            .next()
            .ok_or_eyre("Missing value")?
            .strip_prefix(b"\"")
            .ok_or_eyre("Value missing opening quote")?
            .strip_suffix(b"\"")
            .ok_or_eyre("Value missing close quote")?;
        if kv_parts.next().is_some() {
            bail!("Unexpected data after key value pair");
        }

        match key {
            b"OFXHEADER" => {
                let parsed = str::from_utf8(value)
                    .wrap_err("invalid utf8 in OFXHEADER")
                    .and_then(|v| v.parse::<u32>().wrap_err("Failed to parse OFXHEADER"))?;
                if ofxheader.replace(parsed).is_some() {
                    bail!("Repeated header 'OFXHEADER")
                }
            }
            b"VERSION" => {
                let parsed = str::from_utf8(value)
                    .wrap_err("invalid utf8 in VERSION")
                    .and_then(|v| v.parse::<u32>().wrap_err("Failed to parse VERSION"))?;
                if version.replace(parsed).is_some() {
                    bail!("Repeated header 'VERSION")
                }
            }
            b"SECURITY" => {
                if security {
                    bail!("Repeated header 'SECURITY");
                }
                match value {
                    b"NONE" => security = true,
                    v => bail!("Unrecognized SECURITY value: {:?}", v),
                }
            }
            b"OLDFILEUID" => {
                if oldfileuid {
                    bail!("Repeated header 'OLDFILEUID");
                }
                match value {
                    b"NONE" => oldfileuid = true,
                    v => bail!("Unrecognized OLDFILEUID value: {:?}", v),
                }
            }
            b"NEWFILEUID" => {
                if newfileuid {
                    bail!("Repeated header 'NEWFILEUID");
                }
                match value {
                    b"NONE" => newfileuid = true,
                    v => bail!("Unrecognized NEWFILEUID value: {:?}", v),
                }
            }
            h => bail!("Unrecognized OFX header key: {:?}", h),
        }
    }

    if !security {
        bail!("Header 'SECURITY' missing");
    }
    if !oldfileuid {
        bail!("Header 'OLDFILEUID' missing");
    }
    if !newfileuid {
        bail!("Header 'NEWFILEUID' missing");
    }

    Ok(Header {
        ofxheader: ofxheader.ok_or_eyre("Header 'OFXHEADER' missing")?,
        version: version.ok_or_eyre("Header 'VERSION' missing")?,
        encoding: encoding.ok_or_eyre("XML encoding missing")?,
    })
}
