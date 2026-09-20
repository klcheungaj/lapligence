// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_order.sv
// IEEE 1800-2009 12.7.3: unpacked dimensions precede packed dimensions.
module tb;
    logic [3:0] a [0:1];
    logic [2:1][0:1] matrix [-1:0][3:2];
    integer ones;
    integer visits;
    integer readback;

    initial begin
        a[0] = 4'b1011;
        a[1] = 4'b0101;
        ones = 0;
        foreach (a[i,j]) begin
            $write("%0d:%0d ", i, j);
            ones += a[i][j];
        end
        $display("ones=%0d", ones);

        visits = 0;
        foreach (matrix[i,j,k,l]) begin
            $write("%0d:%0d:%0d:%0d ", i, j, k, l);
            matrix[i][j][k][l] = visits % 2;
            visits++;
        end
        $display("visits=%0d", visits);
        readback = 0;
        foreach (matrix[i,j,k,l]) readback += matrix[i][j][k][l];
        $display("readback=%0d words=%b,%b,%b,%b", readback,
                 matrix[-1][3], matrix[-1][2], matrix[0][3], matrix[0][2]);
    end
endmodule
