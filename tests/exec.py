import subprocess


if __name__ == '__main__':
    subprocess.Popen("python3 ./integrations/checker.py", shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    test = subprocess.Popen("cargo test --release -- --ignored test_http_action::test_http_content_length_replace", shell=True)
    test.wait()
