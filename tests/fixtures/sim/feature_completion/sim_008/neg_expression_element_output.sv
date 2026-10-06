// SIM-008 boundary: an output actual naming a queue element in a function
// call used inside an expression has no statement after the call to store
// the element, so it is rejected explicitly (legal by SV 13.5.2).
module tb;
    int q[$];
    int k;

    function automatic int f(output int x);
        x = 4;
        return 1;
    endfunction

    initial begin
        q = '{0};
        k = f(q[0]) + 1;
        $display("%0d %0d", k, q[0]);
        $finish(0);
    end
endmodule
