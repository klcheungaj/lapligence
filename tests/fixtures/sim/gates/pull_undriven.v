// IEEE 1364-2001 3.7.1 and 7.8: a net with no drivers has the high-impedance
// value Z; pullup and pulldown primitives provide their respective weak value.
module tb;
    wire w_up;
    wire w_down;
    wire w_plain;
    pullup pu(w_up);
    pulldown pd(w_down);

    initial begin
        #1;
        if (w_up !== 1'b1 || w_down !== 1'b0 || w_plain !== 1'bz) begin
            $display("FAIL pull_undriven up=%b down=%b plain=%b",
                     w_up, w_down, w_plain);
            $finish;
        end
        $display("PASS pull_undriven");
        $finish;
    end
endmodule
