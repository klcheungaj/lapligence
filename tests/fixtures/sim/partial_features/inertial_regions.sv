`timescale 1ns/1ns
module tb;
    logic source = 0;
    wire zero, delayed;
    assign #0 zero = source;
    assign #2 delayed = source;
    initial begin
        #0 $display("inactive %b", zero);
        source <= 1;
        $strobe("postponed %b", zero);
        #2 source <= 0;
        $display("active %b", delayed);
        $strobe("settled %b %b", delayed, zero);
        #2 $strobe("later %b", delayed);
        #1 $finish(0);
    end
endmodule
