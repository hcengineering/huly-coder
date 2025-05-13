Remove-Item -Path ./logs -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path ./history.json -Force -ErrorAction SilentlyContinue
Remove-Item -Path ./target/workspace -Force -ErrorAction SilentlyContinue