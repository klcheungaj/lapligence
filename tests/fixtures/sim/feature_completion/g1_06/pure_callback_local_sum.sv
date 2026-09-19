// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_06/pure_callback_local_sum.sv
// IEEE 1800-2009 13.4.1, 13.5.1 and 9.4.2: an automatic function with
// automatic locals, a for loop and a nested eligible call is a legal evaluated
// event expression when it performs no visible write and consumes no time.
module tb;
    int value = 0;
    logic enable = 0;
    int changes = 0;
    int qualifying = 0;

    // 1 + 2 + ... + n using a private accumulator and loop.
    function automatic int triangular(input int n);
        int acc;
        int i;
        acc = 0;
        for (i = 1; i <= n; i = i + 1)
            acc = acc + i;
        return acc;
    endfunction

    // Nested eligible call: 1 exactly when triangular(n) exceeds 4.
    function automatic logic exceeds(input int n);
        exceeds = (triangular(n) > 4);
    endfunction

    always @(triangular(value))
        changes = changes + 1;

    always @(posedge exceeds(value) iff enable)
        qualifying = qualifying + 1;

    initial begin
        #1 value = 1;   // triangular 0 -> 1
        #1 value = 2;   // triangular 1 -> 3
        #1 enable = 1;  // qualifier only
        #1 value = 3;   // triangular 3 -> 6, posedge qualifies
        #1 value = 2;   // triangular 6 -> 3, negedge
        #1 value = 3;   // triangular 3 -> 6, posedge qualifies
        #1 $display("pure_callback_local_sum changes=%0d qualifying=%0d",
                    changes, qualifying);
        $finish(0);
    end
endmodule
