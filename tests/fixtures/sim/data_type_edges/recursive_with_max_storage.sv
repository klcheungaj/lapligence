// IEEE 1800-2009 13.4.2: automatic functions may recurse. The scalar call
// stack remains independent of unrelated maximum-admissible packed storage.
module tb #(parameter WIDTH = 1048575);
    bit [WIDTH-1:0] storage;
    integer recursion_result;
    integer failed;

    function automatic integer recurse(input integer remaining);
        if (remaining == 0)
            recurse = 0;
        else
            recurse = 1 + recurse(remaining - 1);
    endfunction

    initial begin
        failed = 0;
        if (storage !== '0) begin
            $display("FAIL maximum-storage-default WIDTH=%0d", WIDTH);
            failed = 1;
        end
        storage[WIDTH-1] = 1'b1;
        if (!failed && (storage[WIDTH-1] !== 1'b1 || storage[64] !== 1'b0 ||
                        storage[0] !== 1'b0)) begin
            $display("FAIL maximum-storage-high-bit WIDTH=%0d", WIDTH);
            failed = 1;
        end
        recursion_result = recurse(100);
        if (!failed && recursion_result !== 100) begin
            $display("FAIL scalar-recursion WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed) $display("PASS recursive_with_max_storage WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
