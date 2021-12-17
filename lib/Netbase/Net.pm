package Netbase::Net;
use strict;
use warnings;
use utf8;

use Carp qw( croak );
use FFI::Platypus::Buffer qw( grow scalar_to_pointer );
use Netbase;

$Netbase::ffi->mangler( sub { "netbase_net_" . shift } );

$Netbase::ffi->attach(
    new => [ 'string', 'u32', 'u16', 'u32' ] => 'net_t',
    sub {
        my ( $xsub, $class, %args ) = @_;
        my $timeout = delete $args{timeout} // 30;
        my $retry   = delete $args{retry}   // 3;
        my $retrans = delete $args{retrans} // 1;
        if ( %args ) {
            croak "unrecognized arguments: " . join( ' ', sort keys %args );
        }
        $timeout = int( $timeout * 1000 );
        $retrans = int( $retrans * 1000 );
        return $xsub->( $class, $timeout, $retry, $retrans );
    }
);

$Netbase::ffi->attach(
    lookup => [ 'net_t', 'question_t', 'ip_t', 'u64*', 'u32*', '(usize)->opaque' ] => 'u32',
    sub {
        my ( $xsub, $client, $question, $ip ) = @_;
        my $query_start    = 0;
        my $query_duration = 0;
        my $buffer         = "";
        my $closure        = $Netbase::ffi->closure(
            sub {
                my ( $size ) = @_;
                grow( $buffer, $size );
                return Netbase::scalar_to_pointer $buffer;
            }
        );

        my $error = $xsub->( $client, $question, $ip, \$query_start, \$query_duration, $closure );
        if ( $error ) {
            die {
                error          => $Netbase::NUM2ERROR{$error} // $Netbase::E_INTERNAL,
                query_start    => $query_start,
                query_duration => $query_duration,
            };
        }
        else {
            return $buffer, $query_start, $query_duration;
        }
    }
);

$Netbase::ffi->attach( DESTROY => ['net_t'] );

1;
