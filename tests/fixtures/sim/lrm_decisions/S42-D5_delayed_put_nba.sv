// Decision S42-D5: a delayed vpi_put_value (inertial, transport or pure
// transport) updates its object in the NBA region of the target time slot,
// like a nonblocking assignment with an intra-assignment delay: processes in
// the Active and Inactive regions of that slot still read the old value.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64179-64184) gives the
// delay modes but does not say in which region the update happens; the text
// is silent. llg uses the NBA region of 4.4.2.4 (L3158-3161).
//
// Build S42-D5_delayed_put_nba.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d5()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic [3:0] d = 4'd0;
    initial begin
        $d5;
        #2 $display("active d=%0d", d);
        #0 $display("inactive d=%0d", d);
        $strobe("postponed d=%0d", d);
        #1 $finish;
    end
endmodule
