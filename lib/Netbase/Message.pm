package Netbase::Message;
use strict;
use warnings;
use utf8;

use Netbase;

$Netbase::ffi->mangler( sub { "netbase_message_" . shift } );

$Netbase::ffi->attach( to_string => ['message_t'] => 'string' );

$Netbase::ffi->attach( DESTROY => ['message_t'] );

use overload '""' => \&to_string;

1;
