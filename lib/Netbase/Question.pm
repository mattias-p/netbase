package Netbase::Question;
use strict;
use warnings;
use utf8;

use Exporter qw( import );
use Netbase qw( proto rrtype );
use Netbase::Name qw( name );

our @EXPORT_OK = qw( question );

sub question {
    my ( $qname, $qtype, $opts ) = @_;
    $opts //= {};
    my $proto             = $opts->{proto}             // $Netbase::PROTO_UDP;
    my $recursion_desired = $opts->{recursion_desired} // 0;

    $qname = name( $qname )   // return;
    $qtype = rrtype( $qtype ) // return;
    $proto = proto( $proto )  // return;

    return Netbase::Question->new( $qname, $qtype, $proto, $recursion_desired );
}

$Netbase::ffi->mangler( sub { "netbase_question_" . shift } );

$Netbase::ffi->attach( new => [ 'string', 'name_t', 'rrtype_t', 'proto_t', 'u8' ] => 'question_t' );

$Netbase::ffi->attach(
    set_edns => [ 'question_t', 'u8', 'u8', 'u16', 'u8[]', 'usize' ],
    sub {
        my ( $xsub, $this, $version, $dnssec_ok, $option_code, $option_value ) = @_;
        $xsub->( $this, $version, $dnssec_ok, $option_code, $option_value, length $option_value );
    }
);
$Netbase::ffi->attach( to_string => ['question_t'] => 'string' );

$Netbase::ffi->attach( DESTROY => ['question_t'] );

use overload '""' => \&to_string;

1;
