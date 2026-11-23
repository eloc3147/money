use color_eyre::eyre::{Context, OptionExt, Result, bail, eyre};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Text encoding declared by the file header.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StringEncoding {
    /// UTF-8.
    Utf8,
    /// Windows code page 1252.
    Windows1252,
}

/// The parsed OFX file header.
#[derive(Debug)]
pub struct Header {
    /// The `OFXHEADER` version (100 for SGML, 200 for XML).
    pub ofxheader: u32,
    /// The OFX specification version.
    pub version: u32,
    /// Encoding of the body text.
    pub encoding: StringEncoding,
}

/// Read an OFX 1.x (SGML) `KEY:VALUE` header, up to the blank line ending it.
///
/// # Errors
/// Fails on unknown, repeated or missing header fields.
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
                    b"NONE" | b"TYPE1" => security = true,
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

/// Read the two processing-instruction lines of an OFX 2.x (XML) header.
///
/// # Errors
/// Fails on unknown, repeated or missing header fields.
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn sgml(text: &str) -> Result<Header> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("h.qfx");
        std::fs::write(&path, text)?;
        read_sgml_header(&mut BufReader::new(File::open(&path).await?)).await
    }

    async fn xml(text: &str) -> Result<Header> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("h.qfx");
        std::fs::write(&path, text)?;
        read_xml_header(&mut BufReader::new(File::open(&path).await?)).await
    }

    const SGML: &str = "OFXHEADER:100\nDATA:OFXSGML\nVERSION:102\nSECURITY:NONE\nENCODING:USASCII\n\
        CHARSET:1252\nCOMPRESSION:NONE\nOLDFILEUID:NONE\nNEWFILEUID:NONE\n\n";
    const XML: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
        <?OFX OFXHEADER=\"200\" VERSION=\"202\" SECURITY=\"NONE\" OLDFILEUID=\"NONE\" NEWFILEUID=\"NONE\"?>\n";

    #[tokio::test]
    async fn reads_valid_headers() {
        let h = sgml(SGML).await.unwrap();
        assert_eq!(
            (h.ofxheader, h.version, h.encoding),
            (100, 102, StringEncoding::Windows1252)
        );
        let h = xml(XML).await.unwrap();
        assert_eq!(
            (h.ofxheader, h.version, h.encoding),
            (200, 202, StringEncoding::Utf8)
        );
    }

    #[tokio::test]
    async fn rejects_bad_sgml_headers() {
        for (from, to) in [
            ("DATA:OFXSGML", "DATA:XML"),
            ("SECURITY:NONE", "SECURITY:TYPE2"),
            ("ENCODING:USASCII", "ENCODING:UTF8"),
            ("CHARSET:1252", "CHARSET:8859"),
            ("COMPRESSION:NONE", "COMPRESSION:ZIP"),
            ("OLDFILEUID:NONE", "OLDFILEUID:1"),
            ("NEWFILEUID:NONE", "NEWFILEUID:1"),
            ("VERSION:102", "VERSION:x"),
            ("OFXHEADER:100", "OFXHEADER:x"),
            ("OFXHEADER:100", "OFXHEADER:100\nOFXHEADER:100"),
            ("VERSION:102", "VERSION:102\nVERSION:102"),
            ("SECURITY:NONE", "SECURITY:NONE\nSECURITY:NONE"),
            ("DATA:OFXSGML", "DATA:OFXSGML\nDATA:OFXSGML"),
            ("SECURITY:NONE\n", ""),
            ("NEWFILEUID:NONE\n", ""),
            ("OLDFILEUID:NONE\n", ""),
            ("VERSION:102\n", ""),
            ("CHARSET:1252", "NOCOLON"),
            ("CHARSET:1252", "UNKNOWN:1"),
        ] {
            assert!(
                sgml(&SGML.replacen(from, to, 1)).await.is_err(),
                "{from} -> {to}"
            );
        }
    }

    #[tokio::test]
    async fn rejects_bad_xml_headers() {
        for (from, to) in [
            ("version=\"1.0\"", "version=\"1.1\""),
            ("encoding=\"utf-8\"", "encoding=\"latin1\""),
            ("encoding=\"utf-8\"", "standalone=\"yes\""),
            ("encoding=\"utf-8\"", "encoding=utf-8"),
            ("<?xml", "<xml"),
            ("?>\n<?OFX", "\n<?OFX"),
            ("<?OFX", "<OFX"),
            ("SECURITY=\"NONE\"", "SECURITY=\"TYPE1\""),
            ("SECURITY=\"NONE\" ", ""),
            ("OLDFILEUID=\"NONE\" ", ""),
            ("NEWFILEUID=\"NONE\"", ""),
            ("OLDFILEUID=\"NONE\"", "OLDFILEUID=\"1\""),
            ("NEWFILEUID=\"NONE\"", "NEWFILEUID=\"1\""),
            ("VERSION=\"202\"", "VERSION=\"x\""),
            ("OFXHEADER=\"200\"", "OFXHEADER=\"x\""),
            ("OFXHEADER=\"200\"", "OFXHEADER=\"200\" OFXHEADER=\"200\""),
            ("VERSION=\"202\"", "VERSION=\"202\" VERSION=\"202\""),
            ("VERSION=\"202\"", "OTHER=\"1\""),
            ("VERSION=\"202\"", "VERSION=\"202\"=x"),
        ] {
            assert!(
                xml(&XML.replacen(from, to, 1)).await.is_err(),
                "{from} -> {to}"
            );
        }
    }
}
