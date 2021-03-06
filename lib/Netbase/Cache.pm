=head1 NAME

Netbase::Cache - a mapping from requests to responses

=head1 DESCRIPTION

A B<Netbase::Cache> contains a complete record of all requests that have been
sent.
Every request is marked with a time stamp and a duration representing the time
(UTC) when the request was sent and the time taken before a response was
received or an error occurred.
When a request fails and is retried each attempt is recorded and time stamped.

A request is represented by a normalized logical description from which an
actual request can be generated.
Two requests that differ only in what protocol they are sent over are given
distinct representations.

=head2 Cache miss strategies

When a request is made that has a cached response, that response is returned and
no network request.

When there is no cached response Netbase has two strategies for you to choose
from.
Either it gives an error response indicating that the request is not in the
cache, or it transparently sends a network request and records the response in
the cache before returning it.

=cut

package Netbase::Cache;
use strict;
use warnings;
use utf8;

use Carp qw( croak );
use FFI::Platypus::Buffer qw( grow scalar_to_pointer );
use Netbase;
use Netbase::Message;

$Netbase::ffi->mangler( sub { "netbase_cache_" . shift } );

=head1 ERRORS

All these subroutines call foreign code to achieve their task.
In case the foreign code panics (without terminating the process) the native
subroutine reacts by calling C<croak>.

=head1 CONSTRUCTORS

=head2 new

Construct a new empty cache.

    my $cache = Netbase::Cache->new();

=cut

$Netbase::ffi->attach(
    new => [] => 'cache_t',
    sub {
        my ( $xsub, $class ) = @_;

        return $xsub->()    #
          // croak "panic in foreign code\n";
    }
);

=head2 from_bytes

Construct a new cache populated with the deserialized contents of a byte string.

    my $cache = Netbase::Cache->from_bytes( $bytes );

=cut

$Netbase::ffi->attach(
    from_bytes => [ 'buffer', '(usize)->opaque' ] => 'cache_t',
    sub {
        my ( $xsub, $class, $buffer ) = @_;

        my $err_msg  = "";
        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $size ) = @_;
                grow( $err_msg, $size );
                return scalar_to_pointer $err_msg;
            }
        );

        my $cache = $xsub->( $buffer, $closure );
        if ( !defined $cache ) {
            if ( $err_msg eq "" ) {
                croak "panic in foreign code\n";
            }
            else {
                $err_msg .= "\n";
                croak $err_msg;
            }
        }

        return $cache;
    },
);

=head1 METHODS

=head2 to_bytes

Serialize the contents into a byte string.

    my $bytes = $cache->to_bytes();

=cut

$Netbase::ffi->attach(
    to_bytes => [ 'cache_t', '(usize)->opaque' ] => 'u8',
    sub {
        my ( $xsub, $cache ) = @_;

        my $buffer  = "";
        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $size ) = @_;
                grow( $buffer, $size );
                return scalar_to_pointer $buffer;
            }
        );

        $xsub->( $cache, $closure )
          or croak "panic in foreign code\n";

        return $buffer;
    }
);

=head2 lookup

Look up responses to a question from a set of server addresses.

    my $href = $cache->lookup( $net, $question, @ips );
    for my $ip ( keys %$href ) {
        my ( $started, $duration, $msg_size, $error, $message ) = @{ $href->{$ip} };
    }

=cut

$Netbase::ffi->attach(
    lookup => [ 'cache_t', 'opaque', 'question_t', 'opaque[]', 'usize', '(opaque,u64,u32,u16,u16,opaque)->void' ] => 'u8',
    sub {
        my ( $xsub, $cache, $client, $question, @ips ) = @_;

        my %results;
        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $ip, $start, $duration, $err_kind, $msg_size, $message ) = @_;
                $ip = Netbase::opaque_to_ip $ip;
                if ( defined $message ) {
                    $message = Netbase::opaque_to_message $message;
                }
                if ( $err_kind ) {
                    $err_kind = $Netbase::NUM2ERROR{$err_kind} // $Netbase::E_INTERNAL;
                }
                $results{$ip} = [ $start, $duration, $msg_size, $err_kind, $message ];
            }
        );

        if ( defined $client ) {
            $client = Netbase::net_to_opaque $client;
        }

        my @ip_ptrs = map { Netbase::ip_to_opaque $_ } @ips;

        $xsub->( $cache, $client, $question, \@ip_ptrs, scalar @ips, $closure )
          or croak "panic in foreign code\n";

        return \%results;
    }
);

=head2 for_each_request

Traverse all cached requests.

    $cache->for_each_request(
        sub {
            my ( $ip, $question ) = @_;
        }
    );

=cut

$Netbase::ffi->attach(
    for_each_request => [ 'cache_t', '(opaque, opaque)->void' ] => 'u8',
    sub {
        my ( $xsub, $cache, $callback ) = @_;

        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $question, $ip ) = @_;
                $question = Netbase::opaque_to_question $question;
                $ip       = Netbase::opaque_to_ip $ip;
                $callback->( $question, $ip );
            }
        );

        $xsub->( $cache, $closure )
          or croak "panic in foreign code\n";

        return;
    }
);

=head2 for_each_retry

Traverse all cached failed queries for a given request.

    $cache->for_each_retry(
        $question,
        $ip,
        sub {
            my ( $start, $duration, $error ) = @_;
        }
    );

=cut

$Netbase::ffi->attach(
    for_each_retry => [ 'cache_t', 'question_t', 'ip_t', '(u64, u32, u32)->void' ] => 'u8',
    sub {
        my ( $xsub, $cache, $question, $server, $callback ) = @_;

        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $start, $duration, $error ) = @_;
                $error = $Netbase::NUM2ERROR{$error} // $Netbase::E_INTERNAL;
                $callback->( $start, $duration, $error );
            }
        );

        $xsub->( $cache, $question, $server, $closure )
          or croak "panic in foreign code\n";

        return;
    }
);

$Netbase::ffi->attach( DESTROY => ['cache_t'] => 'u8', sub {
    my ( $xsub, $this ) = @_;

    $xsub->( $this )
      or croak "panic in foreign code\n";

    return;
});

1;
