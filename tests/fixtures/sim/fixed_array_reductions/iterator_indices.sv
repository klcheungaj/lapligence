// IEEE 1800-2009 7.12.4: the active fixed unpacked iterator dimension.
module tb;
    logic [7:0] ascending [2:4];
    logic [7:0] descending [1:-1];
    logic [7:0] matrix [1:0][-1:0];
    logic [0:0] narrow_dimension;
    int dynamic_dimension;
    int dimension_calls;
    int ascending_default, ascending_explicit, ascending_dynamic, ascending_narrow_dynamic;
    int descending_explicit, descending_dynamic;
    int nested_named, nested_dynamic;

    function automatic int select_dimension();
        dimension_calls++;
        return dynamic_dimension;
    endfunction

    initial begin
        ascending[2] = 2; ascending[3] = 3; ascending[4] = 4;
        descending[1] = 1; descending[0] = 0; descending[-1] = 8'hff;
        matrix[1][-1] = 5; matrix[1][0] = 10;
        matrix[0][-1] = 15; matrix[0][0] = 20;
        dynamic_dimension = 1;
        narrow_dimension = 1'b1;

        ascending_default = ascending.sum(item) with (item.index());
        ascending_explicit = ascending.sum(item) with (item.index(1));
        dimension_calls = 0;
        ascending_dynamic = ascending.sum(item) with (item.index(select_dimension()));
        ascending_narrow_dynamic = ascending.sum(item) with (item.index(narrow_dimension));
        descending_explicit = descending.sum(item) with (item.index(1));
        descending_dynamic = descending.sum(item) with (item.index(dynamic_dimension));
        nested_named = matrix.sum(row) with (row.sum(cell_value) with (cell_value.index(1) + row.index(1)));
        nested_dynamic = matrix.sum(row) with (row.sum(cell_value) with (cell_value.index(dynamic_dimension)));

        $display("ascending=%0d,%0d,%0d dynamic=%0d narrow=%0d dimension_calls=%0d",
                 ascending_default, ascending_explicit, ascending_dynamic, ascending_dynamic,
                 ascending_narrow_dynamic, dimension_calls);
        $display("descending=%0d dynamic=%0d", descending_explicit,
                 descending_dynamic);
        $display("nested=%0d dynamic=%0d", nested_named, nested_dynamic);
        $finish(0);
    end
endmodule
