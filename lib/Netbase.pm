package Netbase;
use strict;
use warnings;
use utf8;

use strict;
use warnings;
use 5.014;

our $VERSION = '0.01';

use Const::Fast;
use Exporter qw( import );
use FFI::Platypus 1.00;
use Scalar::Util qw( dualvar isdual looks_like_number );

our @EXPORT_OK = qw(
  proto
  rrtype
);

my %NAME2RRTYPE;
my %NUM2RRTYPE;
our %NUM2ERROR;
my %NAME2PROTO;
my %NUM2PROTO;

our $ffi = FFI::Platypus->new( api => 1, lang => 'Rust' );

$ffi->load_custom_type( '::PointerSizeBuffer' => 'buffer' );

$ffi->type( 'object(Netbase::Cache)'    => 'cache_t' );
$ffi->type( 'object(Netbase::Net)'      => 'net_t' );
$ffi->type( 'object(Netbase::IP)'       => 'ip_t' );
$ffi->type( 'object(Netbase::Name)'     => 'name_t' );
$ffi->type( 'object(Netbase::Question)' => 'question_t' );
$ffi->type( 'object(Netbase::Message)'  => 'message_t' );
$ffi->type( 'u16'                       => 'rrtype_t' );
$ffi->type( 'u8'                        => 'proto_t' );

$ffi->attach_cast( 'ip_to_opaque',       'ip_t',   'opaque' );
$ffi->attach_cast( 'net_to_opaque',      'net_t',  'opaque' );
$ffi->attach_cast( 'opaque_to_ip',       'opaque', 'ip_t' );
$ffi->attach_cast( 'opaque_to_message',  'opaque', 'message_t' );
$ffi->attach_cast( 'opaque_to_question', 'opaque', 'question_t' );

$ffi->bundle;

const our $RRTYPE_A          => dualvar 1,   "A";
const our $RRTYPE_AAAA       => dualvar 28,  "AAAA";
const our $RRTYPE_ANY        => dualvar 255, "ANY";
const our $RRTYPE_IXFR       => dualvar 251, "IXFR";
const our $RRTYPE_AXFR       => dualvar 252, "AXFR";
const our $RRTYPE_CAA        => dualvar 257, "CAA";
const our $RRTYPE_CNAME      => dualvar 5,   "CNAME";
const our $RRTYPE_DNSKEY     => dualvar 48,  "DNSKEY";
const our $RRTYPE_DS         => dualvar 43,  "DS";
const our $RRTYPE_HINFO      => dualvar 13,  "HINFO";
const our $RRTYPE_HTTPS      => dualvar 65,  "HTTPS";
const our $RRTYPE_KEY        => dualvar 25,  "KEY";
const our $RRTYPE_MX         => dualvar 15,  "MX";
const our $RRTYPE_NAPTR      => dualvar 35,  "NAPTR";
const our $RRTYPE_NS         => dualvar 2,   "NS";
const our $RRTYPE_NSEC       => dualvar 47,  "NSEC";
const our $RRTYPE_NSEC3      => dualvar 50,  "NSEC3";
const our $RRTYPE_NSEC3PARAM => dualvar 51,  "NSEC3PARAM";
const our $RRTYPE_NULL       => dualvar 10,  "NULL";
const our $RRTYPE_OPENPGPKEY => dualvar 61,  "OPENPGPKEY";
const our $RRTYPE_OPT        => dualvar 41,  "OPT";
const our $RRTYPE_PTR        => dualvar 12,  "PTR";
const our $RRTYPE_RRSIG      => dualvar 46,  "RRSIG";
const our $RRTYPE_SIG        => dualvar 24,  "SIG";
const our $RRTYPE_SOA        => dualvar 6,   "SOA";
const our $RRTYPE_SRV        => dualvar 33,  "SRV";
const our $RRTYPE_SSHFP      => dualvar 44,  "SSHFP";
const our $RRTYPE_SVCB       => dualvar 64,  "SVCB";
const our $RRTYPE_TLSA       => dualvar 52,  "TLSA";
const our $RRTYPE_TSIG       => dualvar 250, "TSIG";
const our $RRTYPE_TXT        => dualvar 16,  "TXT";
const our $RRTYPE_ZERO       => dualvar 0,   "ZERO";

const our $E_INTERNAL => dualvar 1, "INTERNAL_ERROR";
const our $E_IO       => dualvar 2, "IO_ERROR";
const our $E_PROTOCOL => dualvar 3, "PROTOCOL_ERROR";
const our $E_TIMEOUT  => dualvar 4, "TIMEOUT_ERROR";
const our $E_LOCK     => dualvar 5, "LOCK_ERROR";

const our $PROTO_UDP => dualvar 1, "UDP";
const our $PROTO_TCP => dualvar 2, "TCP";

{
    my @all_protos = (    #
        $PROTO_UDP,
        $PROTO_TCP,
    );
    for my $proto ( @all_protos ) {
        $NUM2PROTO{ 0 + $proto } = $proto;
        $NAME2PROTO{"$proto"} = $proto;
        my $name = "\$PROTO_$proto";
        push @EXPORT_OK, $name;
    }

    my @all_errors = (    #
        $E_INTERNAL,
        $E_PROTOCOL,
        $E_IO,
        $E_TIMEOUT,
        $E_LOCK,
    );
    for my $error ( @all_errors ) {
        $NUM2ERROR{ 0 + $error } = $error;
        my $name = "$error" =~ s/(.*)_ERROR/\$E_$1/mr;
        push @EXPORT_OK, $name;
    }

    my @all_rrtypes = (    #
        $RRTYPE_A,
        $RRTYPE_AAAA,
        $RRTYPE_ANY,
        $RRTYPE_IXFR,
        $RRTYPE_AXFR,
        $RRTYPE_CAA,
        $RRTYPE_CNAME,
        $RRTYPE_DNSKEY,
        $RRTYPE_DS,
        $RRTYPE_HINFO,
        $RRTYPE_HTTPS,
        $RRTYPE_KEY,
        $RRTYPE_MX,
        $RRTYPE_NAPTR,
        $RRTYPE_NS,
        $RRTYPE_NSEC,
        $RRTYPE_NSEC3,
        $RRTYPE_NSEC3PARAM,
        $RRTYPE_NULL,
        $RRTYPE_OPENPGPKEY,
        $RRTYPE_OPT,
        $RRTYPE_PTR,
        $RRTYPE_RRSIG,
        $RRTYPE_SIG,
        $RRTYPE_SOA,
        $RRTYPE_SRV,
        $RRTYPE_SSHFP,
        $RRTYPE_SVCB,
        $RRTYPE_TLSA,
        $RRTYPE_TSIG,
        $RRTYPE_TXT,
        $RRTYPE_ZERO,
    );
    for my $rrtype ( @all_rrtypes ) {
        $NAME2RRTYPE{"$rrtype"} = $rrtype;
        $NUM2RRTYPE{ 0 + $rrtype } = $rrtype;
        push @EXPORT_OK, "\$RRTYPE_$rrtype";
    }
}

sub proto {
    my $value = shift;

    if ( looks_like_number( $value ) && $value == "$value" && $value == int( $value ) && $value >= 0 && $value < 256 ) {
        return $NUM2PROTO{$value} // $value;
    }
    elsif ( defined $value && ( my $proto = $NAME2PROTO{ uc $value } ) ) {
        if ( !isdual( $value ) || $value + 0 == 0 || $value + 0 == $proto ) {
            return $proto;
        }
    }

    return;
}

sub rrtype {
    my $value = shift;

    if ( looks_like_number( $value ) && $value == "$value" && $value == int( $value ) && $value >= 0 && $value < 65536 ) {
        return $NUM2RRTYPE{$value} // $value;
    }
    elsif ( my $rrtype = $NAME2RRTYPE{ uc $value } ) {
        if ( !isdual( $value ) || $value + 0 == 0 || $value + 0 == $rrtype ) {
            return $rrtype;
        }
    }

    return;
}

1;
