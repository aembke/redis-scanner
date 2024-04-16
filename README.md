TODO

* build process for most targets, include these in releases, with rustls and native-tls variants
* test process - fake data, scan it
* clap argv
* commands
    * touch
    * inspect idle
    * inspect ttl
    * memory usage

* ideas:
* periodically call dbsize to update the denominator? or just show the number of keys scanned?

shared argv:
--pattern <scan glob>
--filter <regexp>
--group-by <regexp>
--delay <int ms>
--page-size <int>
--cluster --replicas | --sentinel <service>
--tls, --tls-key <path>, --tls-cert <path>, etc
--quiet
--host, --port, --username, --password

formatting argv:
--sort ASC|DESC
--limit <count>
--offset <count>
--format CSV|JSON|Table

commands: touch, idle, ttl, memory-usage