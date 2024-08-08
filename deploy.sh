cargo build --relase
scp target/release/allgreen ubuntu@3.70.135.37
ssh ubuntu@3.70.135.37 "
cd ~
mv allgreen server
sudo systemctl restart server
exit "
