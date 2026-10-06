// SIM-011 boundary: a multidimensional fixed-array class property is legal
// (SV 7.4.2, 8.4) but is rejected with a dedicated diagnostic.
class grid_c;
    int g[2][3];
endclass

module tb;
    grid_c h;

    initial begin
        h = new;
        h.g[1][2] = 5;
        $display("%0d", h.g[1][2]);
    end
endmodule
