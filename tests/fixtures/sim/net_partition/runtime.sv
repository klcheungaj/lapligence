// llg-test-fixture: wide array resolution under repeatedly toggling drivers.
module tb;
    localparam N = 64;
    localparam W = 128;
    logic [W-1:0] a = '0, b = '1;
    wire [W-1:0] resolved[N];
    for (genvar element = 0; element < N; element++) begin : cells
        assign resolved[element] = a;
        assign resolved[element] = b;
    end
    initial begin
        for (int tick = 0; tick < 100; tick++) begin
            a = ~a;
            b = tick[0] ? 'z : ~a;
            #1;
            for (int element = 0; element < N; element++)
                if (resolved[element] !== (tick[0] ? a : {W{1'bx}}))
                    $fatal(1, "wide resolution");
        end
        $display("WIDE_NET_RUNTIME_PASS");
        $finish(0);
    end
endmodule
