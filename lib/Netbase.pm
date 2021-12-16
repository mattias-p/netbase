package Netbase;

use strict;
use warnings;
use 5.014;

our $VERSION = '0.01';

use Const::Fast;
use Exporter qw( import );
use FFI::Platypus 1.00;
use Scalar::Util qw( blessed dualvar isdual looks_like_number );

our @EXPORT_OK = qw(
  ip
  name
  proto
  question
  rrtype
);
our %EXPORT_TAGS = (
    helpers => [
        qw(
          ip
          name
          proto
          question
          rrtype
        )
    ],
);

my %NAME2RRTYPE;
my %NUM2RRTYPE;
my %NUM2ERROR;
my %NAME2PROTO;
my %NUM2PROTO;

my $ffi = FFI::Platypus->new( api => 1, lang => 'Rust' );

$ffi->load_custom_type( '::PointerSizeBuffer' => 'buffer' );

$ffi->type( 'object(Netbase::Cache)'    => 'cache_t' );
$ffi->type( 'object(Netbase::Net)'      => 'net_t' );
$ffi->type( 'object(Netbase::IP)'       => 'ip_t' );
$ffi->type( 'object(Netbase::Name)'     => 'name_t' );
$ffi->type( 'object(Netbase::Question)' => 'question_t' );
$ffi->type( 'object(Netbase::Message)'  => 'message_t' );
$ffi->type( 'u16'                       => 'rrtype_t' );
$ffi->type( 'u8'                        => 'proto_t' );

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
        push @{ $EXPORT_TAGS{proto} }, $name;
    }

    my @all_errors = (    #
        $E_INTERNAL,
        $E_PROTOCOL,
        $E_IO,
        $E_TIMEOUT,
    );
    for my $error ( @all_errors ) {
        $NUM2ERROR{ 0 + $error } = $error;
        my $name = $error =~ s/(.*)_ERROR/\$E_$1/m;
        push @EXPORT_OK, $name;
        push @{ $EXPORT_TAGS{error} }, $name;
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
        push @{ $EXPORT_TAGS{rrtype} }, "\$RRTYPE_$rrtype";
    }

    # add all the other ":class" tags to the ":all" class,
    # deleting duplicates
    {
        my %seen;
        push @{ $EXPORT_TAGS{all} }, grep { !$seen{$_}++ } @{ $EXPORT_TAGS{$_} } foreach keys %EXPORT_TAGS;
    }
}

sub ip {
    my $ip = shift;

    if ( blessed $ip && $ip->isa( 'Netbase::IP' ) ) {
        return $ip;
    }
    return Netbase::IP->new( $ip );
}

sub proto {
    my $value = shift;

    if ( looks_like_number( $value ) && $value == "$value" && $value == int( $value ) && $value >= 0 && $value < 256 ) {
        return $NUM2PROTO{$value} // $value;
    }
    elsif ( my $proto = $NAME2PROTO{ uc $value } ) {
        if ( !isdual( $value ) || $value + 0 == 0 || $value + 0 == $proto ) {
            return $proto;
        }
    }

    return;
}

sub name {
    my $name = shift;

    if ( blessed $name && $name->isa( 'Netbase::Name' ) ) {
        return $name;
    }
    return Netbase::Name->from_ascii( $name );
}

sub question {
    my ( $qname, $qtype, $opts ) = @_;
    $opts //= {};
    my $proto             = $opts->{proto}             // $PROTO_UDP;
    my $recursion_desired = $opts->{recursion_desired} // 0;

    $qname = name( $qname )   // return;
    $qtype = rrtype( $qtype ) // return;
    $proto = proto( $proto )  // return;

    return Netbase::Question->new( $qname, $qtype, $proto, $recursion_desired );
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

package Netbase::Cache;

use FFI::Platypus::Buffer qw( grow scalar_to_pointer );

$ffi->mangler( sub { "netbase_cache_" . shift } );

$ffi->attach( new        => ['string']             => 'cache_t' );
$ffi->attach( from_bytes => [ 'string', 'buffer' ] => 'cache_t' );
$ffi->attach(
    to_bytes => [ 'cache_t', '(usize)->opaque' ],
    sub {
        my ( $xsub, $cache ) = @_;
        my $buffer  = "";
        my $closure = $ffi->closure(
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
$ffi->attach(
    lookup => [ 'cache_t', 'opaque', 'question_t', '(u64,u32,u16,u16,opaque,opaque)->void', 'opaque[]', 'usize' ],
    sub {
        my ( $xsub, $cache, $client, $question, @ips ) = @_;
        my $start    = 0;
        my $duration = 0;
        my $msg_size = 0;
        my $err_kind = 0;
        my $message  = undef;
        my $ip;
        my $closure = $ffi->closure(
            sub {
                ( $start, $duration, $msg_size, $err_kind, $message, $ip ) = @_;
                $ip = $ffi->cast( 'opaque' => 'ip_t', $ip );
                if ( defined $message ) {
                    $message = $ffi->cast( 'opaque' => 'message_t', $message );
                }
            }
        );
        if ( defined $client ) {
            $client = $ffi->cast( 'net_t' => 'opaque', $client );
        }
        if ( $err_kind ) {
            $err_kind = $NUM2ERROR{$err_kind} // $E_INTERNAL;
        }
        my @ip_ptrs = map { $ffi->cast( 'ip_t' => 'opaque', $_ ) } @ips;
        $xsub->( $cache, $client, $question, $closure, \@ip_ptrs, scalar @ips );

        return { $ip => [ $start, $duration, $msg_size, $err_kind, $message ] };
    }
);
$ffi->attach(
    for_each_request => [ 'cache_t', '(opaque, opaque)->void' ],
    sub {
        my ( $xsub, $cache, $callback ) = @_;
        my $closure = $ffi->closure(
            sub {
                my ( $ip, $question ) = @_;
                $ip       = $ffi->cast( 'opaque' => 'ip_t',       $ip );
                $question = $ffi->cast( 'opaque' => 'question_t', $question );
                $callback->( $ip, $question );
            }
        );
        $xsub->( $cache, $closure );
        return;
    }
);
$ffi->attach(
    for_each_retry => [ 'cache_t', 'question_t', 'ip_t', '(u64, u32, u32)->void' ],
    sub {
        my ( $xsub, $cache, $question, $server, $callback ) = @_;
        my $closure = $ffi->closure(
            sub {
                my ( $start, $duration, $error ) = @_;
                $error = $NUM2ERROR{$error} // $E_INTERNAL;
                $callback->( $start, $duration, $error );
            }
        );
        $xsub->( $cache, $question, $server, $closure );
        return;
    }
);
$ffi->attach( DESTROY => ['cache_t'] );

package Netbase::Net;

use Carp qw( croak );
use FFI::Platypus::Buffer qw( grow scalar_to_pointer );

$ffi->mangler( sub { "netbase_net_" . shift } );

$ffi->attach(
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
$ffi->attach(
    lookup => [ 'net_t', 'question_t', 'ip_t', 'u64*', 'u32*', '(usize)->opaque' ] => 'u32',
    sub {
        my ( $xsub, $client, $question, $ip ) = @_;
        my $query_start    = 0;
        my $query_duration = 0;
        my $buffer         = "";
        my $closure        = $ffi->closure(
            sub {
                my ( $size ) = @_;
                grow( $buffer, $size );
                return scalar_to_pointer $buffer;
            }
        );

        my $error = $xsub->( $client, $question, $ip, \$query_start, \$query_duration, $closure );
        if ( $error ) {
            die {
                error          => $NUM2ERROR{$error} // $E_INTERNAL,
                query_start    => $query_start,
                query_duration => $query_duration,
            };
        }
        else {
            return $buffer, $query_start, $query_duration;
        }
    }
);
$ffi->attach( DESTROY => ['net_t'] );

package Netbase::IP;

$ffi->mangler( sub { "netbase_ip_" . shift } );

$ffi->attach( new       => [ 'string', 'string' ] => 'ip_t' );
$ffi->attach( to_string => ['ip_t']               => 'string' );
$ffi->attach( DESTROY   => ['ip_t'] );

use overload '""' => \&to_string;

package Netbase::Name;

$ffi->mangler( sub { "netbase_name_" . shift } );

$ffi->attach( from_ascii => [ 'string', 'string' ] => 'name_t' );
$ffi->attach( to_string  => ['name_t']             => 'string' );
$ffi->attach( DESTROY    => ['name_t'] );

use overload '""' => \&to_string;

package Netbase::Question;

$ffi->mangler( sub { "netbase_question_" . shift } );

$ffi->attach( new => [ 'string', 'name_t', 'rrtype_t', 'proto_t', 'u8' ] => 'question_t' );
$ffi->attach(
    set_edns => [ 'question_t', 'u8', 'u8', 'u16', 'u8[]', 'usize' ],
    sub {
        my ( $xsub, $this, $version, $dnssec_ok, $option_code, $option_value ) = @_;
        $xsub->( $this, $version, $dnssec_ok, $option_code, $option_value, length $option_value );
    }
);
$ffi->attach( to_string => ['question_t'] => 'string' );
$ffi->attach( DESTROY   => ['question_t'] );

use overload '""' => \&to_string;

package Netbase::Message;

$ffi->mangler( sub { "netbase_message_" . shift } );

$ffi->attach( to_string => ['message_t'] => 'string' );
$ffi->attach( DESTROY   => ['message_t'] );

use overload '""' => \&to_string;

1;
