// llg-test-fixture: ADV-020 multichannel descriptors. mcd.c ($mcd,
// $after_close, $vopen, $show_files) opens, names, writes, flushes and
// closes channels through vpi_mcd_* on the same table as
// $fopen/$fdisplay/$fclose, including invalid descriptors, channel 1 and
// file descriptors. Expected output: mcd.out.
module tb;
    integer m, fd, v;
    initial begin
        m = $fopen("adv020_hdl.txt");
        fd = $fopen("adv020_fd.txt", "w");
        $fdisplay(m, "hdl 1");
        $mcd(m, fd);
        $fdisplay(m, "hdl 2");
        $fdisplay(fd, "fd still open");
        $fclose(m);
        $fclose(fd);
        $after_close(m);
        v = $vopen();
        $fdisplay(v, "hdl into a vpi channel");
        $fclose(v);
        $show_files;
        $display("done");
        $finish;
    end
endmodule
