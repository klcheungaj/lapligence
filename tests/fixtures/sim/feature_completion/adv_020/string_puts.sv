// llg-test-fixture: ADV-020 delayed puts to string variables.
// string_puts.c ($sp_put, $sp_cancel) writes the string variables with
// inertial and transport delays and a cancelled event, and checks the forms
// that stay refused (IEEE 1800-2009 38.34). Expected output: string_puts.out.
`timescale 1ns/1ns
class C;
    string p = "p0";
endclass
module tb;
    string s = "s0", t = "t0", u = "u0";
    string q[$] = '{"q0"};
    C c;
    always @(s) $display("%0t: s changed to %s", $time, s);
    initial begin
        c = new;
        #1 $sp_put;
        #1 $sp_cancel;
        #1 $display("%0t: s=%s t=%s u=%s q0=%s p=%s", $time, s, t, u, q[0], c.p);
        #10 $display("%0t: s=%s t=%s u=%s q0=%s p=%s", $time, s, t, u, q[0], c.p);
        $finish;
    end
endmodule
