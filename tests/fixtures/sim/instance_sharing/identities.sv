module shared_leaf;
    event kick;
    integer seen = 0;
    task automatic mark;
        #0;
        seen = seen + 1;
        $display("task %m seen=%0d", seen);
    endtask
    initial begin
        begin : live
            $display("start %m");
            @kick;
            mark();
            #20;
            $display("late %m");
        end
        $display("exit %m");
    end
endmodule

module tb;
    wire [3:0] bits;
    shared_leaf u0();
    shared_leaf u1();
    shared_leaf u2();
    shared_leaf u3();
    for (genvar i = 0; i < 4; i = i + 1) begin : g
        assign bits[i] = i[0];
        initial begin
            #1;
            $display("generate %m");
        end
    end
    initial begin
        #1 -> u0.kick;
        #1 disable u1.live;
        #1;
        -> u2.kick;
        -> u3.kick;
        #1;
        $display("seen %0d %0d %0d %0d bits=%b", u0.seen, u1.seen,
                 u2.seen, u3.seen, bits);
        $finish(0);
    end
endmodule
