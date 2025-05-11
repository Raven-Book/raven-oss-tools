use std::env;
use clap::{Parser, Subcommand};
use raven_oss_tools::crypt::{decrypt_file, encrypt_file, get_crypt_file_name};
use raven_oss_tools::utils::{ensure_absolute_path, UnwrapOrExit};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help(true))]
struct Cli {
    #[command(subcommand)]
    crypt: Option<Crypt>,
}

#[derive(Subcommand, Debug)]
enum Crypt{
    En {
        input_path: String,
        output_path: Option<String>,
        #[arg(short)]
        password: String,
    },
    De {
        input_path: String,
        output_path: Option<String>,
        #[arg(short)]
        password: String,
    }
}

struct RotCrypt {
    input_path: String,
    output_path: Option<String>,
    password: String,
}


async fn _process_crypt_file(rot_crypt: RotCrypt, is_encrypt: bool) -> String {

    let input_path = ensure_absolute_path(&rot_crypt.input_path)
        .unwrap_or_exit("无效的文件路径");

    let filename = get_crypt_file_name(&input_path, is_encrypt).unwrap_or_exit("无法获取文件名");

    let output_path = if let Some(value) = rot_crypt.output_path {
        ensure_absolute_path(&value)
            .unwrap_or_exit("无效的文件路径")
    } else {
        let mut tmp = env::current_dir().expect("failed to get file");
        tmp.push(filename.clone());
        tmp
    };


    if is_encrypt {
        encrypt_file(input_path, output_path, rot_crypt.password).await;
    } else {
        decrypt_file(input_path, output_path, rot_crypt.password).await;
    }
    filename
}


async fn _encrypt(rot_crypt: RotCrypt) {
    let filename = _process_crypt_file(rot_crypt, true).await;
}

async fn _decrypt(rot_crypt: RotCrypt) {
    let filename = _process_crypt_file(rot_crypt, false).await;

}


#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Some(crypt) = cli.crypt {
        match crypt {
            Crypt::En{ input_path, output_path, password } => {
                _encrypt(RotCrypt { input_path, output_path, password }).await;
            }
            Crypt::De { input_path, output_path, password } => {
                _decrypt(RotCrypt{input_path, output_path, password}).await;
            }
        }
    }
}