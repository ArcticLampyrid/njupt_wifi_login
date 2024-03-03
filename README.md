# NJUPT WiFi Login
A tool to help NJUPTer to connect the campus network conveniently.

## Goals
- User-unaware automatic login
- Highly simulate manual operation
- Proxy-friendly
- Low privacy exposure outside of school networks

## How it works?
It will listen for the network changed notifications and automatically do the authentication.

## How to use it?
### Use Configurator
*Configurator is available in v0.1.1 and afterwards.*
1. Download the binaries or build from the source on your own.
2. Open `njupt_wifi_login_configurator`, write down your account and click Save button.
3. Reboot your computer

### Manually configure
1. Download the binaries or build from the source on your own.
2. Write down your userid and password into the configuration file (eg. `njupt_wifi.yml`).
   ```yaml
   isp: CT # 移动用 CMCC，电信用 CT，南邮自身的用 EDU
   userid: "B22999999"
   password: "password123456"
   ```
3. Config to run `njupt_wifi_login` at startup and it will automatically do the rest.

## Requirements
Currently it's Windows-only since the author doesn't use Linux in desktop environments. 

For those who may be interested in Linux support, try to use `NETLINK_ROUTE` to implement the listener. The cargo package [`rtnetlink`](https://github.com/little-dude/netlink/tree/master/rtnetlink) may be helpful. 

PRs for narrowing the limitation is welcome.

## Remarks
It will use no proxy during the authentication for the proxy may be not available until the network is logged in.

It will use specific DNS Servers (in the white list of the firewall) internally to avoid dns not available during authentication.

We write it meticulously with Rust, thus you are mostly not needed to worry about the cost of performance.

## Privacy Concerns
This tool is meticulous in design to lower the privacy exposure when connecting to non-NJUPT networks.

Account information is only sent when the NJUPT AP Portal's certificate is valid, thus preventing account information from being stolen.

Before certificate verification, the initial network traffic sent is a connectivity check to connect.rom.miui.com, which results in a DNS request and an HTTP plaintext request. In most cases, this is the only traffic leaked to an unknown network environment. Considering that all Xiaomi phones send similar connectivity check requests, this hardly reveals any information. 

The worst-case scenario is that the adversary forges the result of the connectivity check and returns a redirect request that appears to come from the NJUPT AP Portal gateway. In this case, a DNS request to p.njupt.edu.cn is leaked, which may expose you as an NJUPT student. However, since the adversary cannot impersonate the NJUPT AP Portal (lacking the certificate), the account information will not be sent.

## License
Licensed under [MIT license](LICENSE.txt).