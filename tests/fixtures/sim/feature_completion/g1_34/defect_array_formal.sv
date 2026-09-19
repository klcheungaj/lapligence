// llg-test-fixture: G1-34 rtl_no_silent_omissions (defect witness, ignored).
// IEEE 1800-2009 13.4.2/13.5.1: a fixed unpacked array is a legal input formal
// and the callee reads its elements by value. The current lowerer reports
// `cannot resolve array base of select` for the formal (collection/arguments.rs,
// collection/signatures.rs ownership).
module tb;
    logic [7:0] arr [0:3];

    function automatic int sum_arr(input logic [7:0] v [0:3]);
        sum_arr = v[0] + v[1] + v[2] + v[3];
    endfunction

    initial begin
        arr = '{8'h1, 8'h2, 8'h3, 8'h4};
        $display("sum=%0d", sum_arr(arr));
        $finish(0);
    end
endmodule
