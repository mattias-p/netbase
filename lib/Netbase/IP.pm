package Netbase::IP;
use strict;
use warnings;
use utf8;

use Exporter qw( import );
use Netbase;
use Scalar::Util qw( blessed );

our @EXPORT_OK = qw( ip );

sub ip {
    my $ip = shift;

    if ( blessed $ip && $ip->isa( 'Netbase::IP' ) ) {
        return $ip;
    }
    return Netbase::IP->new( $ip );
}

$Netbase::ffi->mangler( sub { "netbase_ip_" . shift } );

$Netbase::ffi->attach( new => [ 'string', 'string' ] => 'ip_t' );

$Netbase::ffi->attach( to_string => ['ip_t'] => 'string' );

$Netbase::ffi->attach( DESTROY => ['ip_t'] );

use overload '""' => \&to_string;

1;
