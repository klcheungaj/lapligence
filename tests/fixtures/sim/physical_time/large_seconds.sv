`timescale 10s/1s
module ten_seconds(output wire done);
    reg state;
    assign done = state;
    initial begin
        state = 0;
        #1 state = 1;
    end
endmodule

`timescale 100s/1s
module hundred_seconds(output wire done);
    reg state;
    assign done = state;
    initial begin
        state = 0;
        #1 state = 1;
    end
endmodule

`timescale 1s/1fs
module tb;
    wire ten_done, hundred_done;
    ten_seconds u_ten(ten_done);
    hundred_seconds u_hundred(hundred_done);
    always @(ten_done) if (ten_done) $display("ten-global=%0t", $time);
    always @(hundred_done) if (hundred_done) $display("hundred-global=%0t", $time);
    initial begin
        #101 $finish(0);
    end
endmodule
