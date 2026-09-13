`timescale 1ns/1ns
module tb;
    logic [128:0] delay_value;
    logic q=0;
    initial begin
        q<=1;
        delay_value='x;
        #delay_value $display("x %0d %b",$time,q);
        delay_value='z;
        #delay_value $display("z %0d %b",$time,q);
        #(1'bx) $display("literal %0d %b",$time,q);
        delay_value=1;
        #delay_value $display("later %0d %b",$time,q);
        $finish(0);
    end
endmodule
