# Netbase

Netbase is short for network database.

## Dependencies

To build netbase you need the following:
* FFI::Build::MM
* FFI::Build::File::Rust
* rustc >= 1.56.0

## Install

```sh
perl Makefile.PL
sudo make install
```

## Run

These are the major ways to access the documentation:

```sh
netbase --help
netbase --help query
netbase --man
```

## Scope

Netbase has two major features.
It sends simple network (DNS) requests and keeps a cache of already sent
requests.

Netbase has two major interfaces.
One Perl API and one CLI tool.
The Perl API is meant to be used from Zonemaster Engine.
The CLI tool exercises the Perl API and provides a convenient way to inspect and
work with saved cache files without having to parse them yourself or go through
the entire machinery of Zonemaster Engine.

The scope of Netbase is somewhat similar to Zonemaster::LDNS but there are
important differences.
Netbase does more in the sense that it integrates a cache.
But it also does less in the sense that it does not implement fallbacks between
protocols to handle truncation.

## Features

### Cache

The cache is basically a mapping from requests to responses.

It contains a complete record of all requests that have been sent.
Every request is marked with a time stamp and a duration representing the time
(UTC) when the request was sent and the time taken before a response was
received or an error occurred.
When a request fails and is retried each attempt is recorded and time stamped.

A request is represented by a normalized logical description from which an
actual request can be generated.
Two requests that differ only in what protocol they are sent over are given
distinct representations.

### Cache miss strategies

When a request is made that has a cached response, that response is returned and
no network request.

When there is no cached response Netbase has two strategies for you to choose
from.
Either it gives an error response indicating that the request is not in the
cache, or it transparently sends a network request and records the response in
the cache before returning it.

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
* Listing all requests in a cache file.
* Dumping the complete contents of a cache file.

### In progress
* Proper RDATA formatting for all record types.
* Configurable EDNS header and fields. The EDNS version, DO flag and option code
  are done.
  The only thing missing is setting the Z flags.
  N.B. the support for setting option codes is limited, but sufficient for
  Zonemaster Engine.
* The CLI of the tool could probably use some tweaking before it's declared
  stable, but I feel pretty good about its general shape.

### ToDo

#### Feature parity
* Support setting the source address in requests.
  (https://github.com/bluejekyll/trust-dns/pull/1586)
* Support setting EDNS Z flags in requests.
  (https://docs.rs/trust-dns-client/latest/trust_dns_client/op/struct.Edns.html)
* Review the implemented feature set.
  Could Netbase support Zonemaster Engine without adding additional features?
* Add accessors for all parts of a DNS response that we need.

#### Robustness
* Make FFI robust with regard to panics in the Rust code.
  (https://metacpan.org/pod/FFI::Platypus::Lang::Rust#panics)
* Review FFI with regard to advice on object-based APIs.
  (https://rust-unofficial.github.io/patterns/patterns/ffi/export.html)
* What happens if we `die` in Perl callbacks called from Rust?
  Could it be undefined behavior?
* Add tooling for finding memory errors. (Use https://valgrind.org/ or
  something.)

#### Maintainability
* Revisit the naming of things to make it more consistent. Today there are little
  messes around at least lookups/queries/questions/requests, server/ns/ip and
  outcomes/responses/results.
* Revisit all parts of the Rust code and add unit tests for everything.

#### Future work
* Update the FFI to accommodate making lookups to multiple servers with
  identical requests in a single call.
  (I've got some code to perform such lookups in parallel.
  The only part missing is getting the results across the FFI jump.)
* Reusing TCP connections.
* Add a query parameter to delete all records from the answer, authority and
  additional sections. (To be used with AXFR requests.)
* Add support for ASN lookups.
  Both the Cymru and Ripe protocols.
