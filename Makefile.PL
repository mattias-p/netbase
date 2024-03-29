use ExtUtils::MakeMaker;
use FFI::Build::MM;
use FFI::Platypus::Lang::Rust;

my $fbmm = FFI::Build::MM->new;

WriteMakefile(
    $fbmm->mm_args(
        ABSTRACT           => 'A network database library and tool',
        DISTNAME           => 'Netbase',
        NAME               => 'Netbase',
        VERSION_FROM       => 'lib/Netbase.pm',
        CONFIGURE_REQUIRES => {
            'FFI::Build::MM' => '1.00',
        },
        BUILD_REQUIRES => {
            'FFI::Build::MM'          => '1.00',
            'FFI::Build::File::Cargo' => '0.07',
        },
        PREREQ_PM => {
            'Const::Fast'               => '0.014',
            'FFI::Platypus'             => '1.00',
            'FFI::Platypus::Lang::Rust' => '0.07',
        },
        TEST_REQUIRES => {
            'Test2::V0' => '0',
        },
        EXE_FILES => [
            'script/zcache',
        ],
    )
);

sub MY::postamble {
    $fbmm->mm_postamble;
}
