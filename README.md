# Netbase

## Status

### Done

* The design of the tool CLI feels rather solid to me.

### ToDo

#### Feature parity
* Add accessors for all parts of a DNS response that we need.
* Support setting the source address in requests.
  (https://github.com/bluejekyll/trust-dns/pull/1586)
* Support setting EDNS Z flags in requests.
  (https://docs.rs/trust-dns-client/latest/trust_dns_client/op/struct.Edns.html)
* Review the implemented feature set. Could netbase be integrated into
  Zonemaster Engine without adding additional features?

#### Robustness
* Make FFI robust with regard to panics in the Rust code.
  (https://metacpan.org/pod/FFI::Platypus::Lang::Rust#panics)
* Review FFI with respect to adviced on object-based APIs.
  (https://rust-unofficial.github.io/patterns/patterns/ffi/export.html)
* What happens of we throw exceptions in Perl callbacks called from Rust.
  Is this undefined behavior?
* Add tooling for finding memory errors. (Use https://valgrind.org/ or
  something.)

#### Maintainability
* Revisit the naming of things to make it more consistent. Today there are little
  messes around at least lookups/queries/questions/requests, server/ns/ip and
  outcomes/responses/results.
* Revisit all parts of the Rust code and add unit tests for everything.

#### Wish list
* Update the FFI to accomodate lookups to multiple servers using a single
  question in a single call. (I've got some code to perform such lookups in
  parallel. The only part missing is thte FFI jump.)


## Compile

```sh
perl Makefile.PL
make all
```

## Run

```sh
perl -Ilib script/netbase --help
```
