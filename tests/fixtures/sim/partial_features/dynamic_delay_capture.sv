`timescale 1ns/1ns
module tb;
    int delay_value=3;
    int calls;
    function automatic int next_delay(); calls=calls+1; return 2; endfunction
    task automatic pause(input int duration);
        #(duration+1) $display("task %0d",$time);
    endtask
    initial begin
        #delay_value $display("first %0d",$time);
        delay_value=2;
        #(delay_value+1) $display("second %0d",$time);
        #(next_delay()) $display("call %0d %0d",$time,calls);
        pause(2);
        $finish(0);
    end
    initial #1 delay_value=9;
endmodule
