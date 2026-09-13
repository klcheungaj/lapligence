`timescale 1ns/1ns
module tb;
    real source=1.25, result;
    shortreal narrow;
    int delay_value=2;
    initial begin
        result=#delay_value source;
        $display("real %0d %.2f",$time,result);
        delay_value=1;
        narrow=#delay_value 16777217.0;
        $display("short %0d %.0f",$time,narrow);
        $finish(0);
    end
    initial begin #1; source=9.0; delay_value=7; end
endmodule
