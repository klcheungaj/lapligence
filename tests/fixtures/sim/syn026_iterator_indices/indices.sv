// IEEE 1800-2009 7.12.2-7.12.4: iterator indices use declared bounds and
// each with clause has its own lexical iterator.
module tb;
    int ascending [2:4];
    int descending [3:1];
    int matrix [1:0][-2:-1];
    int cube [1:0][-1:0][3:4];
    int ascending_sum, descending_sum, nested_sum, cube_sum;

    initial begin
        ascending[2] = 30; ascending[3] = 10; ascending[4] = 20;
        descending[3] = 30; descending[2] = 10; descending[1] = 20;
        matrix[1][-2] = 11; matrix[1][-1] = 12;
        matrix[0][-2] = 21; matrix[0][-1] = 22;
        cube[1][-1][3] = 0; cube[1][-1][4] = 0;
        cube[1][0][3] = 0; cube[1][0][4] = 0;
        cube[0][-1][3] = 0; cube[0][-1][4] = 0;
        cube[0][0][3] = 0; cube[0][0][4] = 0;

        ascending_sum = ascending.sum(item) with (item.index);
        descending_sum = descending.sum(item) with (item.index(1));
        nested_sum = matrix.sum(row) with
            (row.sum(cell_value) with (row.index(1) + cell_value.index()));
        cube_sum = cube.sum(plane) with
            (plane.sum(row) with
                (row.sum(cell_value) with
                    (plane.index() + row.index(1) + cell_value.index())));
        $display("indices=%0d,%0d,%0d,%0d",
                 ascending_sum, descending_sum, nested_sum, cube_sum);

        ascending.rsort(item) with (item.index);
        descending.sort(item) with (item.index(1));
        matrix.sort(row) with (row.index(1));
        $display("ascending=%0d,%0d,%0d descending=%0d,%0d,%0d",
                 ascending[2], ascending[3], ascending[4],
                 descending[3], descending[2], descending[1]);
        $display("rows=%0d,%0d;%0d,%0d",
                 matrix[1][-2], matrix[1][-1], matrix[0][-2], matrix[0][-1]);
        $finish(0);
    end
endmodule
