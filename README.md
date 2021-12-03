# Netbase

Netbase is short for network database.

## Scope

Netbase has two major features.
It sends simple network (DNS) requests and keeps a cache of already sent
requests.

Netbase has two major interfaces.
One Perl API and one CLI tool.
The Perl API is meant to be used from Zonemaster Engine.
The CLI tool exercises the Perl API and allows people to inspect and work with
saved cache files without having to go through the entire machinery of
Zonemaster Engine.

The scope of Netbase is somewhat similar to Zonemaster::LDNS but there are
important differences.
Netbase does more in the sense that it performs caching of performed requests.
But it also does less in the sense that it does not implement fallbacks between
protocols to handle truncation.

## Cache

The cache contains a mapping from requests to responses.

The cache contains a complete record of all requests that have been sent.
Every request is marked with a time stamp and a duration representing the time
(UTC) when the request was sent and the time taken before either a response was
received or an error occurred.
In case failed requests are retried each one of those attemts are recorded.

A request is represented by a normalized logical description from which an
actual request can be generated.
Two requests that differ only in what protocol they are sent over are given
distinct representations.

## Install

```sh
perl Makefile.PL
make install
```

## Run

```sh
netbase --help
netbase --help query
netbase --man
```

## Status

### Done (beta quality)
* FFI calls from Perl to Rust. (CLI tool in Perl, cache and networking in Rust.)
* DNS requests over UDP and TCP.
* Configurable timeout waiting for requests.
* Retrying failed requests with a delay in between tries.
* Configurable qname, qtype and RD flag in requests.
* Saving and loading cache files.
* Making lookups against the cache only. (I.e. without making network requests.)
* Usage documenation for all implemented features in the CLI tool.

### In progress
* The CLI of the tool could probably use some tweaking, but I feel good about
  its general shape.
* Configurable EDNS header and fields. The EDNS version, DO flag and option code
  are done. The only thing missing is setting the Z flags. N.B. the support for
  setting option codes is limited, but sufficient for Zonemaster Engine.

### ToDo

#### Feature parity
* Add accessors for all parts of a DNS response that we need.
* Support setting the source address in requests.
  (https://github.com/bluejekyll/trust-dns/pull/1586)
* Support setting EDNS Z flags in requests.
  (https://docs.rs/trust-dns-client/latest/trust_dns_client/op/struct.Edns.html)
* Review the implemented feature set. Could netbase support Zonemaster Engine
  without adding additional features?

#### Robustness
* Make FFI robust with regard to panics in the Rust code.
  (https://metacpan.org/pod/FFI::Platypus::Lang::Rust#panics)
* Review FFI with respect to adviced on object-based APIs.
  (https://rust-unofficial.github.io/patterns/patterns/ffi/export.html)
* What happens if we throw exceptions in Perl callbacks called from Rust.
  Is this undefined behavior?
* Add tooling for finding memory errors. (Use https://valgrind.org/ or
  something.)

#### Maintainability
* Revisit the naming of things to make it more consistent. Today there are little
  messes around at least lookups/queries/questions/requests, server/ns/ip and
  outcomes/responses/results.
* Revisit all parts of the Rust code and add unit tests for everything.

#### Future work
* Update the FFI to accomodate lookups to multiple servers using a single
  question in a single call. (I've got some code to perform such lookups in
  parallel. The only part missing is thte FFI jump.)
* Reusing TCP connections.
* Add a question parameter to delete all records from the answer, authority and
  additional sections.
* Add support for ASN lookups. Both the Cymru and Ripe protocols.
