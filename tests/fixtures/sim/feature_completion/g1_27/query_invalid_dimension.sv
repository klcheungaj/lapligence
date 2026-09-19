// IEEE 1800-2009 20.7: a runtime dimension outside 1..$dimensions returns 'x
// without indexing outside the descriptor; a legal runtime dimension returns
// the descriptor's current metadata.
module tb;
    logic [7:0] arr [3:0];
    integer dynamic[];
    integer dim;

    initial begin
        dynamic = new[2];
        dim = 0;
        $display("zero left=%0d size=%0d", $left(arr, dim), $size(arr, dim));
        dim = 3;
        $display("high left=%0d size=%0d", $left(arr, dim), $size(arr, dim));
        dim = 1;
        $display("dyn1 left=%0d size=%0d", $left(dynamic, dim), $size(dynamic, dim));
        dim = 3;
        $display("dyn3 left=%0d size=%0d", $left(dynamic, dim), $size(dynamic, dim));
        $display("PASS query_invalid_dimension");
        $finish(0);
    end
endmodule
