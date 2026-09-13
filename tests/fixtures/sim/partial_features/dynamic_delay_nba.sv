`timescale 1ns/100ps
module tb;
    int delay_value, index;
    logic source;
    logic [3:0] q=0;
    real real_delay, real_source, result;
    initial begin
        delay_value=3; index=0; source=1;
        q[index]<=#delay_value source;
        delay_value=1; index=1;
        q[index]<=#delay_value source;
        real_delay=0.5; real_source=2.5;
        result<=#real_delay real_source;
        delay_value=9; index=2; source=0; real_source=9.0;
        $display("issued %0d %b",$time,q);
        #2 $display("early %0d %b %.1f",$time,q,result);
        #2 $display("late %0d %b",$time,q);
        $finish(0);
    end
endmodule
