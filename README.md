# Wake-On-LAN Server

![image](./image.png)

## What is it?

This App sends a magic packet to a specified mac address.
I'm running this app on a raspberry pi 3 with ubuntu 20.04.
I have also installed [tailscale](https://tailscale.com/) to access this app via VPN and boot my PC from an external network.

## Run on Ubuntu 20.04

```
# install dependencies
sudo apt update
sudo apt install python3-pip python3-dev build-essential libssl-dev libffi-dev python3-setuptools
sudo apt install python3-venv nginx

# setup app
cd wakeonlan-server
python3 -m venv env
source env/bin/activate
pip install -r requirements.txt
sudo vi /etc/systemd/system/wakeonlan-server.service # write with reference to the example
sudo systemctl start wakeonlan-server

# setup nginx
sudo vi /etc/nginx/sites-available/wakeonlan-server # write with reference to the example
sudo ln -s /etc/nginx/sites-available/wakeonlan-server /etc/nginx/sites-enabled
sudo unlink /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl restart nginx
```

if you can access http://{ip address}/wol, setup complete.

## References

https://www.digitalocean.com/community/tutorials/how-to-serve-flask-applications-with-uwsgi-and-nginx-on-ubuntu-20-04
