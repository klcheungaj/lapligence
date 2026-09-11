`timescale 10ns/100ps
module child;
    real delay_value=0.025;
    initial #delay_value $display("child %.3f",$realtime);
endmodule
`timescale 1ns/100ps
module tb;
    real delay_value=0.24;
    child dut();
    initial begin
        #delay_value $display("first %.3f",$realtime);
        delay_value=0.25;
        #delay_value $display("second %.3f",$realtime);
        delay_value=0.05;
        #delay_value $display("third %.3f",$realtime);
        delay_value=0.049;
        #delay_value $display("zero %.3f",$realtime);
        $finish;
    end
endmodule
