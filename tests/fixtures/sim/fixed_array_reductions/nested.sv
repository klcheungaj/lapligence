// Each row is an immediate element. Each nested with has a distinct iterator.
module tb;
    logic [7:0] matrix [1:0][-1:0];
    int total, named, indexed;
    initial begin
        matrix[1][-1] = 5; matrix[1][0] = 10;
        matrix[0][-1] = 15; matrix[0][0] = 20;
        total = matrix.sum() with (item.sum() with (int'(item)));
        named = matrix.sum(row) with (row.sum(cell_value) with (int'(cell_value)));
        indexed = matrix.sum(row) with (row.sum(cell_value) with (int'(cell_value) + row.index()));
        $display("nested=%0d named=%0d indexed=%0d", total, named, indexed);
        $display("bounds=%0d,%0d", matrix.sum(row) with (row.index(1)),
                 matrix[1].sum(value) with (value.index()));
        $finish(0);
    end
endmodule
