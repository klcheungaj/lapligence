// A NaN real delay has no 64-bit tick count: the process waits one tick,
// with a warning (decision S01-D1).
module tb;
    real delay_value, argument;
    initial begin
        argument=-1.0;
        delay_value=$sqrt(argument);
        #delay_value;
        $display("after %0t", $realtime);
        $finish(0);
    end
endmodule
