// Equal-strength wired tables; generated hierarchical sites keep independent slots.
module wired_leaf #(parameter W=7) (input wire [W-1:0] local_value);
    wand [W-1:0] and_bus;
    wor [W-1:0] or_bus;
    assign and_bus = local_value;
    assign or_bus = local_value;
endmodule
module wired_check #(parameter W=7) (output reg done);
    reg [W-1:0] local_value, parent_value, extra_value;
    integer a, b, c, n, events, before_events;
    reg expected_and, expected_or;
    genvar index;
    generate for (index=0; index<2; index=index+1) begin : g
        wired_leaf #(W) u(local_value);
    end endgenerate
    assign g[0].u.and_bus = parent_value;
    assign g[0].u.or_bus = parent_value;
    assign g[1].u.and_bus = extra_value;
    assign g[1].u.or_bus = extra_value;
    generate if (W > 1) begin : split
        assign g[0].u.and_bus[W-1:1] = extra_value[W-1:1];
        assign g[0].u.and_bus[0] = extra_value[0];
        assign g[0].u.or_bus[W-1:1] = extra_value[W-1:1];
        assign g[0].u.or_bus[0] = extra_value[0];
        assign {g[1].u.and_bus[W-1:1], g[1].u.or_bus[0]} = parent_value;
        assign g[1].u.and_bus[0] = parent_value[0];
        assign g[1].u.or_bus[W-1:1] = parent_value[W-1:1];
    end else begin : scalar
        assign g[0].u.and_bus = extra_value;
        assign g[0].u.or_bus = extra_value;
        assign g[1].u.and_bus = parent_value;
        assign g[1].u.or_bus = parent_value;
    end endgenerate

    function state;
        input integer code;
        begin case (code) 0: state=0; 1: state=1; 2: state=1'bx; 3: state=1'bz; endcase end
    endfunction
    function and_three;
        input x, y, z;
        begin
            if (x===0 || y===0 || z===0) and_three=0;
            else if (x===1'bx || y===1'bx || z===1'bx) and_three=1'bx;
            else if (x===1 || y===1 || z===1) and_three=1;
            else and_three=1'bz;
        end
    endfunction
    function or_three;
        input x, y, z;
        begin
            if (x===1 || y===1 || z===1) or_three=1;
            else if (x===1'bx || y===1'bx || z===1'bx) or_three=1'bx;
            else if (x===0 || y===0 || z===0) or_three=0;
            else or_three=1'bz;
        end
    endfunction
    task fail;
        input [255:0] reason;
        begin $display("FAIL wired W=%0d %0s", W, reason); $finish(0); end
    endtask
    always @(g[0].u.and_bus) events=events+1;
    initial begin
        done=0; events=0;
        local_value={W{1'bz}}; parent_value={W{1'bz}}; extra_value={W{1'bz}};
        #1;
        for (a=0; a<4; a=a+1)
            for (b=0; b<4; b=b+1)
                for (c=0; c<4; c=c+1) begin
                    for (n=0; n<W; n=n+1) begin
                        local_value[n]=state((a+n)%4);
                        parent_value[n]=state((b+2*n)%4);
                        extra_value[n]=state((c+3*n)%4);
                    end
                    #1;
                    for (n=0; n<W; n=n+1) begin
                        expected_and=and_three(local_value[n], parent_value[n], extra_value[n]);
                        expected_or=or_three(local_value[n], parent_value[n], extra_value[n]);
                        if (g[0].u.and_bus[n] !== expected_and || g[1].u.and_bus[n] !== expected_and ||
                            g[0].u.or_bus[n] !== expected_or || g[1].u.or_bus[n] !== expected_or)
                            fail("driver table or selected contribution");
                    end
                end
        local_value={W{1'b0}}; parent_value={W{1'b1}}; extra_value={W{1'bz}};
        #1; before_events=events;
        parent_value={W{1'bx}};
        #1;
        if (events != before_events || g[0].u.and_bus !== {W{1'b0}})
            fail("no resolved change must not publish an event");
        local_value={W{1'bz}}; parent_value={W{1'bz}}; extra_value={W{1'bz}};
        #1;
        if (g[0].u.and_bus !== {W{1'bz}} || g[1].u.or_bus !== {W{1'bz}})
            fail("all contributions removed");
        done=1;
    end
endmodule
module tb;
    wire [3:0] done;
    wired_check #(1) c1(done[0]);
    wired_check #(7) c7(done[1]);
    wired_check #(65) c65(done[2]);
    wired_check #(129) c129(done[3]);
    initial begin
        wait (&done);
        $display("WIRED_MATRIX_PASS");
        $finish(0);
    end
endmodule
