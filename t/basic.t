use Test2::V0;
use Test2::Tools::Class;

use Netbase qw( ip name proto question rrtype $RRTYPE_A $RRTYPE_AAAA $RRTYPE_NS $RRTYPE_SOA );
use Scalar::Util qw( dualvar );

subtest 'Netbase' => sub {
    subtest 'exported rrtype' => sub {
        is "" . $RRTYPE_NS, "NS";
        is 0 + $RRTYPE_NS, 2;
    };

    subtest 'rrtype from string' => sub {
        is "" . rrtype( "NS" ), "NS";
        is 0 + rrtype( "NS" ), 2;
    };

    subtest 'rrtype from number' => sub {
        is "" . rrtype( 2 ), "NS";
        is 0 + rrtype( 2 ), 2;
    };

    subtest 'rrtype from rrtype' => sub {
        is "" . rrtype( rrtype( "NS" ) ), "NS";
        is 0 + rrtype( rrtype( "NS" ) ), 2;
    };

    subtest 'invalid rrtypes' => sub {
        is rrtype( "FOOBAR" ), undef, qr/unrecognized/, 'undefined record type name';
        is rrtype( -1 ), undef, qr/unrecognized/, 'out of range';
        is rrtype( 65537 ), undef, qr/unrecognized/, 'out of range';
        is rrtype( 2.1 ), undef, qr/unrecognized/, 'fractional number';
        is rrtype( dualvar(1, "NS") ), undef, qr/unrecognized/, 'inconsistent dual number';
    };
};

subtest 'Netbase::IP' => sub {
    subtest 'new()' => sub {
        my $ip1 = Netbase::IP->new( "192.0.2.1" );
        isa_ok $ip1, ['Netbase::IP'], 'returns an instance';
    };

    subtest 'Netbase::Util::ip()' => sub {
        my $ip1 = ip( "192.0.2.1" );
        isa_ok $ip1, ['Netbase::IP'], 'returns an instance';

        my $ip2 = ip( ip( "192.0.2.1" ) );
        isa_ok $ip2, ['Netbase::IP'], 'accepts ip as Netbase::IP';
    };

    subtest 'stringification' => sub {
        my $ip1 = ip( "192.0.2.1" );
        is $ip1->to_string(), "192.0.2.1", 'to_string() returns correct string';
        is "$ip1", "192.0.2.1", 'q("") returns correct string';
    };
};

subtest 'Netbase::Name' => sub {
    subtest 'new()' => sub {
        my $name = Netbase::Name->from_ascii( "example.com" );
        isa_ok $name, ['Netbase::Name'], 'returns an instance';
    };

    subtest 'Netbase::Util::name()' => sub {
        my $name1 = name( "example.com" );
        isa_ok $name1, ['Netbase::Name'], 'returns an instance';

        my $name2 = name( name( "example.com" ) );
        isa_ok $name2, ['Netbase::Name'], 'accepts name as Netbase::Name';
    };

    subtest 'stringification' => sub {
        my $name = name( "example.com" );
        is $name->to_string(), "example.com", 'to_string() returns correct string';
        is "$name", "example.com", 'q("") returns correct string';
    };
};

subtest 'Netbase::Question' => sub {
    subtest 'new()' => sub {
        isa_ok( Netbase::Question->new( name( "example.com" ), rrtype( "A" ), proto( "UDP" ) ), ['Netbase::Question'], 'returns an instance' );
    };

    subtest 'Netbase::Util::question()' => sub {
        my $question1 = question( name( "example.com" ), "A", { proto => "UDP" } );
        isa_ok $question1, ['Netbase::Question'], 'returns an instance';

        my $question2 = question( "example.com", "A", { proto => "UDP" } );
        isa_ok $question2, ['Netbase::Question'], 'accepts name as string';
    };

    subtest 'stringification' => sub {
        my $question = question( "example.com", "A", { proto => "UDP" } );
        is $question->to_string(), "example.com A +norecurse +udp", 'to_string() returns correct string';
        is "$question", "example.com A +norecurse +udp", 'q("") returns correct string';
    };
};

subtest 'Netbase::Net' => sub {
    subtest 'new()' => sub {
        my $net = Netbase::Net->new();
        isa_ok $net, ['Netbase::Net'], 'returns an instance';
    };
};

subtest 'Netbase::Cache' => sub {
    subtest 'new()' => sub {
        my $cache = Netbase::Cache->new();
        isa_ok $cache, ['Netbase::Cache'], 'returns an instance';
    };

    subtest 'lookup()' => sub {
        my $cache = Netbase::Cache->new();
        my ($response, $start, $duration) = $cache->lookup( undef, question('example.com', 'A'), ip( '192.0.2.1' ) );
        is $response, undef, "returns undef (not found)";
        is $start, 0, "returns start time 0";
        is $duration, 0, "returns query duration 0";
    };

    subtest '{from,to}_bytes()' => sub {
        my $net = Netbase::Net->new();
        my $cache1 = Netbase::Cache->new();
        my $buffer1 = $cache1->to_bytes();
        my $cache2 = Netbase::Cache->from_bytes($buffer1);
        isa_ok $cache2, ['Netbase::Cache'], 'returns an instance';
        $cache2->lookup( $net, question('paivarinta.se', 'A'), ip( '9.9.9.9' ));
        my $buffer2 = $cache2->to_bytes();
        isnt $buffer2, $buffer1;
        my $cache3 = Netbase::Cache->from_bytes($buffer2);
        my $buffer3 = $cache3->to_bytes();
        is $buffer3, $buffer2;
    };
};

done_testing;
