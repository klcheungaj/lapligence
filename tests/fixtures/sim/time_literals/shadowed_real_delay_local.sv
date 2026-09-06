module tb;
    parameter real P = 0.25;
    initial begin : local_scope
        real P;
        #P $finish;
    end
endmodule
