### Tools
```
brew install streamingfast/tap/substreams
brew install streamingfast/tap/substreams-sink-sql
```


### Debug
Add logs, rebuild, see `replay.log` file or GUI output
```
substreams gui substreams.yaml --stop-block +10 --start-block 439866040
```
