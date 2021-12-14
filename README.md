# Netbase

A library and CLI tool to make cached DNS and ASN lookups.

N.B. ASN lookups aren't implemented yet.

Netbase revolves around two central ideas.
First, that every network request should be recorded in the cache.
And second, that the cache miss strategy should be configurable to either simply
fail or to block while it's making a network request to populate the cache entry
before returning.

Netbase is short for network database.

## Dependencies

To build netbase you need these:
* FFI::Build::MM
* FFI::Build::File::Rust
* [cargo/rustc] (rustc >= 1.56)

In case you need to build FFI::Build::MM yourself you probably need these too:
* gcc
* OpenSSL development header

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

## Rationale

For several years the Zonemaster team has been throwing around the idea of
speeding up Zonemaster by reimplementing parts of it in a compiled language.
The part where reimplementation would make the most sense has been identified as
the low-level parts around networking in Zonemaster Engine.
I.e. the part to focus reimplementation on would be the caching.

I'm believer in the Unix philosophy to do one thing and do it well.
I'm also a believer in small steps.
So the scope of Netbase does not stretch above caching.

While already at the time of writing the general design of the caching
implementation is very good there are a few wrinkles.

* Neither AXFR requests nor ASN lookups are included in the cache.
  This is problematic for the unit tests as well as when you want to save a
  recorded cache for later analysis.

* Timed out or otherwise failed request that result in retries are not include
  in the cache.
  Having this information in plain sight would be helpful when investigating
  behaviors of both the network and of Zonemaster Engine itself.

* There is no convenient way to list or dump the contents of a saved cache file.
  This would be very helpful when investigating why Zonemaster makes a certain
  analysis.

* There is no convenient way to make lookups into a cache file for specific
  requests or to update it with single requests.
  This would be useful in various development and troubleshooting contexts.

Solving the first two wrinkles is easy if we're reimplementing these parts of
the code anyway.

Solving the last two wrinkles at this point is arguably easier than not solving
them.
Having those lets us try out the API with simple requests while it's being
developed.

### Technology choices

When it came to the choice of language I simply settled on Rust.
It's very performant, very reliable and if you ask me it's generally nice to be
around.

Once the C API is well defined and stable we should stop and think about whether
Rust is the compiled language we want to use.
If we decide it is then good.
In case we aren't sure we can just reimplement the C API using a different
language.
This should be considerably easier than remaking this entire proof-of-concept
from scratch.

When it came to the choice of DNS library I opted to use trust_dns instead of
ldns which we're happy with in Zonemaster today.
To call ldns from Rust we'd have to create Rust bindings for it.
Using trust_dns was simply easier to get started with.
Since we're starting out with trust_dns we should evaluate that first.
If trust_dns doesn't measure up we still have the option to create Rust bindings
for good old ldns.

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

## Architecture

Netbase consists of two interfaces: a library and a CLI tool.

The CLI tool implements a few use cases that exercises all of the library API.

The library consists of three parts.

1. The business logic.
   This is where the actual functionality lives.
   It is implemented in safe Rust.
   Notably it uses trust_dns rather than ldns for making DNS requests.

   Another option would be to add Rust bindings for ldns so we can continue to
   use that.

2. The C API.
   This part defines C bindings for the business logic and exports them from a
   shared object.
   It is implemented in unsafe Rust.

3. The Perl API.
   This defines idiomatic Perl bindings for the C API.
   It is implemented in in pure Perl.
   It uses FFI::Platypus to attach the exported functions and perform the
   necessary type conversions.

   Another option would be to implement this part in XS and bypass
   FFI::Platypus.
   That would likely end up being slightly more performant but considerably more
   difficult and error prone.

### Integration with Zonemaster

Netbase reimplements a small part of Zonemaster Engine and replaces most but not
all of Zonemaster LDNS.

The part of LDNS that is left out from Netbase is the fallback mechanism for
switching protocols when a truncated response is received.
It's helpful to have a record of the truncated response when debugging, so this
mechanism is best implemented above the caching layer.
Since Netbase is all about the caching it seems like a bad idea to add features
below the Netbase interface but above the cache layer.

#### Stage 1: Networking

At this stage we replace the querying of normal requests to use trust_dns
instead of ldns.
I.e. non-AXFR requests.

Update Zonemaster::Engine::Nameserver::query to:
* Call Netbase::Net::query for the actual query and take care of the returned
  wire formatted response.
* Construct a Zonemaster::Engine::Packet using the response.
* Implement the TC-flag fallback.

Make a benchmark of the performance difference between making queries with ldns
and trust_dns penalized with extra deserialization.

#### Stage 2: Packet inspection

At this stage we make Zonemaster Engine use trust_dns's native response
representation.

* Implement Perl bindings for all parts of trust_dns needed to replace
  Zonemaster::LDNS::Packet.
* Update Zonemaster::Engine::Nameserver::query to not deserialize wire formatted
  responses.
* Without changing the public interface of Zonemaster::Engine::Packet, replace
  its implementation with Netbase.

Make a benchmark of the performance difference between making queries with ldns
and trust_dns without penalization.

#### Stage 3: Caching

At this stage we replace the caching code with a compiled version.

* Without changing the public interface of Zonemaster::Engine::Nameserver and
  Zonemaster::Engine::Nameserver::Cache, replace their implementations to use
  Netbase::Cache instead.

Make a benchmark of the performance difference between the pure Perl and
compiled implementations of the cache.

#### Stage 4: AXFR

At this stage we replace the querying of AXFR requests to use trust_dns instead
of ldns.

* Update Zonemaster::Engine::Nameserver::axfr to use Netbase::Cache instead of
  Zonemaster::LDNS.

This stage is not about performance and should not affect it much, but why not
make a benchmark of it anyway?

#### Stage 5: Cleaning up

At this stage we make Zonemaster Engine independent of Zonemaster LDNS.

Remove Zonemaster LDNS as a dependency from Zonemaster Engine.

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

[cargo/rustc]: https://rustup.rs/
