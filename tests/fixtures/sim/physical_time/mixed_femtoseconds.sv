`timescale 1fs/1fs
module one_fs;
    initial begin
        #1 $display("one=%0.0f", $realtime);
    end
endmodule

`timescale 10fs/1fs
module ten_fs;
    initial begin
        #1 $display("ten=%0.0f", $realtime);
    end
endmodule

`timescale 100fs/10fs
module hundred_fs;
    initial begin
        #1 $display("hundred=%0.0f", $realtime);
    end
endmodule

`timescale 1ps/100fs
module one_ps;
    initial begin
        #1 $display("ps=%0.0f", $realtime);
    end
endmodule

`timescale 1ns/100fs
module one_ns;
    initial begin
        #1 $display("ns=%0.0f", $realtime);
    end
endmodule

module tb;
    one_fs u_one();
    ten_fs u_ten();
    hundred_fs u_hundred();
    one_ps u_ps();
    one_ns u_ns();
    initial begin
        #2ns $finish(0);
    end
endmodule
