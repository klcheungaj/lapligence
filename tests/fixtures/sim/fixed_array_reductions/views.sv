module tb;
    logic [7:0] matrix [1:0][3:1];
    int calls, selected, sliced, slice_indices;
    function automatic int row();
        calls++;
        return 0;
    endfunction
    initial begin
        calls = 0;
        matrix[1][3] = 10; matrix[1][2] = 20; matrix[1][1] = 30;
        matrix[0][3] = 1; matrix[0][2] = 2; matrix[0][1] = 3;
        selected = matrix[row()].sum();
        sliced = matrix[1][3:2].sum();
        slice_indices = matrix[1][3:2].sum(v) with (v.index());
        $display("selected=%0d calls=%0d slice=%0d indices=%0d", selected, calls, sliced, slice_indices);
    end
endmodule
