package Netbase::Cache;
use strict;
use warnings;
use utf8;

use FFI::Platypus::Buffer qw( grow scalar_to_pointer );
use Netbase;
use Netbase::Message;

$Netbase::ffi->mangler( sub { "netbase_cache_" . shift } );

$Netbase::ffi->attach( new => ['string'] => 'cache_t' );

$Netbase::ffi->attach( from_bytes => [ 'string', 'buffer' ] => 'cache_t' );

$Netbase::ffi->attach(
    to_bytes => [ 'cache_t', '(usize)->opaque' ],
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
        $xsub->( $cache, $closure );
        return $buffer;
    }
);

$Netbase::ffi->attach(
    lookup => [ 'cache_t', 'opaque', 'question_t', '(u64,u32,u16,u16,opaque,opaque)->void', 'opaque[]', 'usize' ],
    sub {
        my ( $xsub, $cache, $client, $question, @ips ) = @_;
        my %results;
        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $start, $duration, $msg_size, $err_kind, $message, $ip ) = @_;
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
        $xsub->( $cache, $client, $question, $closure, \@ip_ptrs, scalar @ips );

        return \%results;
    }
);

$Netbase::ffi->attach(
    for_each_request => [ 'cache_t', '(opaque, opaque)->void' ],
    sub {
        my ( $xsub, $cache, $callback ) = @_;
        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $ip, $question ) = @_;
                $ip       = Netbase::opaque_to_ip $ip;
                $question = Netbase::opaque_to_question $question;
                $callback->( $ip, $question );
            }
        );
        $xsub->( $cache, $closure );
        return;
    }
);

$Netbase::ffi->attach(
    for_each_retry => [ 'cache_t', 'question_t', 'ip_t', '(u64, u32, u32)->void' ],
    sub {
        my ( $xsub, $cache, $question, $server, $callback ) = @_;
        my $closure = $Netbase::ffi->closure(
            sub {
                my ( $start, $duration, $error ) = @_;
                $error = $Netbase::NUM2ERROR{$error} // $Netbase::E_INTERNAL;
                $callback->( $start, $duration, $error );
            }
        );
        $xsub->( $cache, $question, $server, $closure );
        return;
    }
);

$Netbase::ffi->attach( DESTROY => ['cache_t'] );

1;
