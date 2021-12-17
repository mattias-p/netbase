package Netbase::Name;
use strict;
use warnings;
use utf8;

use Exporter qw( import );
use Netbase;
use Scalar::Util qw( blessed );

our @EXPORT_OK = qw( name );

sub name {
    my $name = shift;

    if ( blessed $name && $name->isa( 'Netbase::Name' ) ) {
        return $name;
    }
    return Netbase::Name->from_ascii( $name );
}

$Netbase::ffi->mangler( sub { "netbase_name_" . shift } );

$Netbase::ffi->attach( from_ascii => [ 'string', 'string' ] => 'name_t' );

$Netbase::ffi->attach( to_string => ['name_t'] => 'string' );

$Netbase::ffi->attach( DESTROY => ['name_t'] );

use overload '""' => \&to_string;

1;
