// llg-test-fixture: ADV-020 A01 symbol parity. symbols.c takes the address of
// every routine IEEE 1800-2009 Annex K (vpi_user.h) and Annex M
// (sv_vpi_user.h) declare, so the plugin loads (or the static archive links)
// only when the model defines all of them, then checks that the routines llg
// does not provide yet fail through vpi_chk_error. Expected output:
// symbols.out.
module tb;
    initial begin
        $symbols;
        $finish;
    end
endmodule
