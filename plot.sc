#!/usr/bin/env amm

import coursierapi.MavenRepository

val lkrollcom = MavenRepository.of("https://dl.bintray.com/lkrollcom/maven");

interp.repositories() ++= Seq(lkrollcom);

@

import ammonite.ops._
import ammonite.ops.ImplicitWd._
//import $ivy.`com.lihaoyi::scalatags:0.6.2`, scalatags.Text.all._
//import $ivy.`org.sameersingh::scalaplot:0.0.4`, org.sameersingh.scalaplot.Implicits._
import $ivy.`com.panayotis.javaplot:javaplot:0.5.0`, com.panayotis.gnuplot.{plot => gplot, utils => gutils, _}
import $ivy.`com.github.tototoshi::scala-csv:1.3.5`, com.github.tototoshi.csv._
import $file.build
import java.io.File
import $file.benchmarks, benchmarks.{BenchmarkImpl, implementations}
import $ivy.`se.kth.benchmarks::benchmark-suite-runner:0.3.0-SNAPSHOT`, se.kth.benchmarks.runner._, se.kth.benchmarks.runner.utils._, kompics.benchmarks.benchmarks._
//import build.{relps, relp, binp}

val results = pwd / 'results;
val plots = pwd / 'plots;

@main
def main(show: Boolean = false, skip: Seq[String] = Nil): Unit = {
	if (!(exists! results)) {
		println("No results to plot.");
		return;
	}
	if (exists! plots) {
		rm! plots;
	}
	mkdir! plots;
  if (skip != Nil) {
    println(s"Skipping implementations: ${skip.mkString(", ")}");
  }
	(ls! results).foreach(plotRun(_, show, skip.toSet));
}

private def plotRun(run: Path, show: Boolean, skip: Set[String]): Unit = {
	println(s"Plotting Run '${run}'");
	val summary = run / 'summary;
	val output = plots / run.last;
	mkdir! output;
	(ls! summary).filter(_.toString.endsWith(".data")).foreach(plotData(_, output, show, skip));
}

private def plotData(data: Path, output: Path, show: Boolean, skip: Set[String]): Unit = {
	println(s"Plotting Data '${data}'");
	print("Loading data...");
	//val rawData = read.lines! data;
	val reader = CSVReader.open(data.toIO);
	val rawData = reader.allWithHeaders();
	reader.close()
	println("done");
	//println(rawData);
	val mean = rawData.map(m => (m("IMPL"), m("PARAMS"), m("MEAN").toDouble)).filterNot(t => skip.contains(t._1));
	val meanGrouped = mean.groupBy(_._1).map { case (key, entries) =>
		val params = entries.map(_._2);
		val means = entries.map(_._3);
		(key -> ImplGroupedResult(implementations(key).label, params, means))
	};
	//println(meanGrouped);
	val fileName = data.last.toString.replace(".data", "");
	val benchO = Benchmarks.benchmarkLookup.get(fileName);
	benchO match {
		case Some(bench) => {
			fileName match {
				case "PINGPONG"|"NETPINGPONG" => plotBenchPP(bench, meanGrouped, output, show)
				case "TPPINGPONG"|"NETTPPINGPONG" => plotBenchTPPP(bench, meanGrouped, output, show)
				case "ATOMICREGISTER" => plotBenchAtomicReg(bench, meanGrouped, output, show)
				case "STREAMINGWINDOWS" => plotBenchSW(bench, meanGrouped, output, show)
				case "FIBONACCI" => plotBenchFib(bench, meanGrouped, output, show)
				case "CHAMENEOS" => plotBenchCham(bench, meanGrouped, output, show)
				case "APSP" => plotBenchAPSP(bench, meanGrouped, output, show)
				case symbol => println(s"Could not find instructions for '${symbol}'! Skipping plot.")
			}
		}
		case None => {
			println(s"Could not get benchmark entry for symbol '${fileName}'! Skipping plot.");
		}
	}
	println(s"Finished with '${data}'");
}

private def plotBenchPP(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
	val bench = b.asInstanceOf[BenchmarkWithSpace[PingPongRequest]];
	val space = bench.space.asInstanceOf[ParameterSpacePB[PingPongRequest]];
	val meanGroupedParams = res.mapValues(_.mapParams(space.paramsFromCSV));
	val meanGroupedLong = meanGroupedParams.mapValues(_.map2D(_.numberOfMessages));
	val p = new JavaPlot();
	if (!show) {
		val outfile = output / s"${b.symbol}.eps";
		val epsf = new terminal.PostscriptTerminal(outfile.toString);
		epsf.setColor(true);
        p.setTerminal(epsf);
	}
	p.getAxis("x").setLabel("#msgs");
	p.getAxis("y").setLabel("execution time (ms)");
	p.setTitle(b.name);
	meanGroupedLong.foreach { case (key, res) =>
		val dsp = new gplot.DataSetPlot(res);
		dsp.setTitle(res.implLabel);
		val ps = new style.PlotStyle(style.Style.LINESPOINTS);
		val colour = colourMap(key);
		ps.setLineType(colour);
		ps.setPointType(pointMap(key));
		dsp.setPlotStyle(ps);
		p.addPlot(dsp);
	}
	p.plot();
}

private def plotBenchFib(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
	val bench = b.asInstanceOf[BenchmarkWithSpace[FibonacciRequest]];
	val space = bench.space.asInstanceOf[ParameterSpacePB[FibonacciRequest]];
	val meanGroupedParams = res.mapValues(_.mapParams(space.paramsFromCSV));
	val meanGroupedLong = meanGroupedParams.mapValues(_.map2D(_.fibNumber));
	val p = new JavaPlot();
	if (!show) {
		val outfile = output / s"${b.symbol}.eps";
		val epsf = new terminal.PostscriptTerminal(outfile.toString);
		epsf.setColor(true);
        p.setTerminal(epsf);
	}
	p.getAxis("x").setLabel("Fibonacci Index");
	val yAxis = p.getAxis("y")
  yAxis.setLabel("execution time (ms)");
  yAxis.setLogScale(true);
	p.setTitle(b.name);
	meanGroupedLong.foreach { case (key, res) =>
		val dsp = new gplot.DataSetPlot(res);
		dsp.setTitle(res.implLabel);
		val ps = new style.PlotStyle(style.Style.LINESPOINTS);
		val colour = colourMap(key);
		ps.setLineType(colour);
		ps.setPointType(pointMap(key));
		dsp.setPlotStyle(ps);
		p.addPlot(dsp);
	}
	p.plot();
}

private def plotBenchSW(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
	val bench = b.asInstanceOf[BenchmarkWithSpace[StreamingWindowsRequest]];
	val space = bench.space.asInstanceOf[ParameterSpacePB[StreamingWindowsRequest]];
	val meanGroupedParamsTime = res.mapValues(_.mapParams(space.paramsFromCSV));
	val meanGroupedParams = ImplResults.mapData(
		meanGroupedParamsTime,
		(req: StreamingWindowsRequest, meanTime: Double) => {
			val totalWindows = req.numberOfWindows * req.numberOfPartitions;
			val windowTime = meanTime/totalWindows;
			windowTime
		}
	);
  {
    val meanGroupedNumberOfPartitions = ImplResults.slices(
      meanGroupedParams,
      (req: StreamingWindowsRequest) => (req.batchSize, req.windowSize, req.numberOfWindows, req.windowSizeAmplification),
      (req: StreamingWindowsRequest) => req.numberOfPartitions
    );
    meanGroupedNumberOfPartitions.foreach { case (params, impls) =>
      val p = new JavaPlot();
      if (!show) {
        val outfile = output / s"${b.symbol}-batchsize${params._1}-windowsize${params._2}-numwindows${params._3}-amplification${params._4}.eps";
        val epsf = new terminal.PostscriptTerminal(outfile.toString);
        epsf.setColor(true);
            p.setTerminal(epsf);
      }
      p.getAxis("x").setLabel("#partitions");
      p.getAxis("y").setLabel("average wall time per window (ms)");
      p.setTitle(s"${b.name} (batch = ${params._1} msgs, window = ${params._2}, #windows = ${params._3}, window amp. = ${params._4})");
      impls.foreach { case (key, res) =>
        val dsp = new gplot.DataSetPlot(res);
        dsp.setTitle(res.implLabel);
        val ps = new style.PlotStyle(style.Style.LINESPOINTS);
        val colour = colourMap(key);
        ps.setLineType(colour);
        ps.setPointType(pointMap(key));
        dsp.setPlotStyle(ps);
        p.addPlot(dsp);
      }
      p.plot();
    }
  }
  {
    val meanGroupedWindowSize = ImplResults.slices(
      meanGroupedParams,
      (req: StreamingWindowsRequest) => (req.numberOfPartitions, req.batchSize, req.windowSize, req.numberOfWindows),
      (req: StreamingWindowsRequest) => {
      	val lengthSec = scala.concurrent.duration.Duration(req.windowSize).toUnit(java.util.concurrent.TimeUnit.SECONDS);
      	val unamplifiedSize = lengthSec * 0.008; // 8kB/s
      	val amplifiedSize = (req.windowSizeAmplification.toDouble * unamplifiedSize);
      	(amplifiedSize * 1000.0).round
      }
    );
    meanGroupedWindowSize.foreach { case (params, impls) =>
      val p = new JavaPlot();
      if (!show) {
        val outfile = output / s"${b.symbol}-numpartitions${params._1}-batchsize${params._2}-windowsize${params._3}-numwindows${params._4}.eps";
        val epsf = new terminal.PostscriptTerminal(outfile.toString);
        epsf.setColor(true);
            p.setTerminal(epsf);
      }
      val xAxis = p.getAxis("x");
      xAxis.setLabel("window size (kB)");
      xAxis.setLogScale(true);
      p.getAxis("y").setLabel("average wall time per window (ms)");
      p.setTitle(s"${b.name} (#partitions = ${params._1}, batch = ${params._2} msgs, window = ${params._3}, #windows = ${params._4})");
      impls.foreach { case (key, res) =>
        val dsp = new gplot.DataSetPlot(res);
        dsp.setTitle(res.implLabel);
        val ps = new style.PlotStyle(style.Style.LINESPOINTS);
        val colour = colourMap(key);
        ps.setLineType(colour);
        ps.setPointType(pointMap(key));
        dsp.setPlotStyle(ps);
        p.addPlot(dsp);
      }
      p.plot();
    }
  }
}

private def plotBenchAtomicReg(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
  val bench = b.asInstanceOf[BenchmarkWithSpace[AtomicRegisterRequest]];
  val space = bench.space.asInstanceOf[ParameterSpacePB[AtomicRegisterRequest]];
  val meanGroupedParamsTime = res.mapValues(_.mapParams(space.paramsFromCSV));
  val meanGroupedParams = ImplResults.mapData(
    meanGroupedParamsTime,
    (req: AtomicRegisterRequest, meanTime: Double) => {
      val totalMessages = req.numberOfKeys;
      val throughput = (totalMessages.toDouble*1000.0)/meanTime; // msgs/s
      throughput
    }
  );
  {
    val meanGroupedPipelining = ImplResults.slices(
      meanGroupedParams,
      (req: AtomicRegisterRequest) => (req.readWorkload, req.writeWorkload, req.partitionSize),
      (req: AtomicRegisterRequest) => req.numberOfKeys
    );
    meanGroupedPipelining.foreach { case (params, impls) =>
      val p = new JavaPlot();
      if (!show) {
        val outfile = output / s"${b.symbol}-read${params._1}-write${params._2}-partitionsize-${params._3}.eps";
        val epsf = new terminal.PostscriptTerminal(outfile.toString);
        epsf.setColor(true);
            p.setTerminal(epsf);
      }
      p.getAxis("x").setLabel("#operations");
      p.getAxis("y").setLabel("throughput (operations/s)");
      p.setTitle(s"${b.name} (read workload = ${params._1}, write workload = ${params._2}, partition size = ${params._3})");
      impls.foreach { case (key, res) =>
        val dsp = new gplot.DataSetPlot(res);
        dsp.setTitle(res.implLabel);
        val ps = new style.PlotStyle(style.Style.LINESPOINTS);
        val colour = colourMap(key);
        ps.setLineType(colour);
        ps.setPointType(pointMap(key));
        dsp.setPlotStyle(ps);
        p.addPlot(dsp);
      }
      p.plot();
    }
  }

}

private def plotBenchCham(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
  val bench = b.asInstanceOf[BenchmarkWithSpace[ChameneosRequest]];
  val space = bench.space.asInstanceOf[ParameterSpacePB[ChameneosRequest]];
  val meanGroupedParamsTime = res.mapValues(_.mapParams(space.paramsFromCSV));
  val meanGroupedParams = ImplResults.mapData(
    meanGroupedParamsTime,
    (req: ChameneosRequest, meanTime: Double) => {
      val chameneos = req.numberOfChameneos.toDouble;
      val throughput = (req.numberOfMeetings.toDouble*1000.0)/(meanTime * chameneos); // meetings/s per chameneo
      throughput
    }
  );
  {
    val meanGroupedMeetings = ImplResults.slices(
      meanGroupedParams,
      (req: ChameneosRequest) => Tuple1(req.numberOfMeetings),
      (req: ChameneosRequest) => req.numberOfChameneos
    );
    meanGroupedMeetings.foreach { case (params, impls) =>
      val p = new JavaPlot();
      if (!show) {
        val outfile = output / s"${b.symbol}-meetings${params._1}.eps";
        val epsf = new terminal.PostscriptTerminal(outfile.toString);
        epsf.setColor(true);
            p.setTerminal(epsf);
      }
      val xAxis = p.getAxis("x");
      xAxis.setLabel("#chameneos");
      xAxis.setLogScale(false);
      p.getAxis("y").setLabel("avg. throughput (meetings/s) per chameneo");
      p.setTitle(s"${b.name} (total #meetings = ${params._1})");
      impls.foreach { case (key, res) =>
        val dsp = new gplot.DataSetPlot(res);
        dsp.setTitle(res.implLabel);
        val ps = new style.PlotStyle(style.Style.LINESPOINTS);
        val colour = colourMap(key);
        ps.setLineType(colour);
        ps.setPointType(pointMap(key));
        dsp.setPlotStyle(ps);
        p.addPlot(dsp);
      }
      p.plot();
    }
  }
}

private def plotBenchAPSP(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
  val bench = b.asInstanceOf[BenchmarkWithSpace[APSPRequest]];
  val space = bench.space.asInstanceOf[ParameterSpacePB[APSPRequest]];
  val meanGroupedParams = res.mapValues(_.mapParams(space.paramsFromCSV));
  {
    val meanGroupedNodes = ImplResults.slices(
      meanGroupedParams,
      (req: APSPRequest) => Tuple1(req.blockSize),
      (req: APSPRequest) => req.numberOfNodes
    );
    meanGroupedNodes.foreach { case (params, impls) =>
      val p = new JavaPlot();
      if (!show) {
        val outfile = output / s"${b.symbol}-blocksize${params._1}.eps";
        val epsf = new terminal.PostscriptTerminal(outfile.toString);
        epsf.setColor(true);
            p.setTerminal(epsf);
      }
      p.getAxis("x").setLabel("#nodes in the graph (|V|)");
      p.getAxis("y").setLabel("execution time (ms)");
      p.setTitle(s"${b.name} (block size = ${params._1})");
      impls.foreach { case (key, res) =>
        val dsp = new gplot.DataSetPlot(res);
        dsp.setTitle(res.implLabel);
        val ps = new style.PlotStyle(style.Style.LINESPOINTS);
        val colour = colourMap(key);
        ps.setLineType(colour);
        ps.setPointType(pointMap(key));
        dsp.setPlotStyle(ps);
        p.addPlot(dsp);
      }
      p.plot();
    }
  }
  {
    val meanGroupedWorkers = ImplResults.slices(
      meanGroupedParams,
      (req: APSPRequest) => Tuple1(req.numberOfNodes),
      (req: APSPRequest) => {
      	val entriesSingleDim = req.numberOfNodes / req.blockSize;
      	val totalWorkers = entriesSingleDim*entriesSingleDim;
      	totalWorkers
      }
    );
    meanGroupedWorkers.foreach { case (params, impls) =>
      val p = new JavaPlot();
      if (!show) {
        val outfile = output / s"${b.symbol}-nodes${params._1}.eps";
        val epsf = new terminal.PostscriptTerminal(outfile.toString);
        epsf.setColor(true);
            p.setTerminal(epsf);
      }
      val xAxis = p.getAxis("x");
      xAxis.setLabel("#blocks");
      xAxis.setLogScale(true);
      val yAxis = p.getAxis("y");
      yAxis.setLabel("execution time (ms)");
      yAxis.setLogScale(true);
      p.setTitle(s"${b.name} (|V| = ${params._1})");
      impls.foreach { case (key, res) =>
        val dsp = new gplot.DataSetPlot(res);
        dsp.setTitle(res.implLabel);
        val ps = new style.PlotStyle(style.Style.LINESPOINTS);
        val colour = colourMap(key);
        ps.setLineType(colour);
        ps.setPointType(pointMap(key));
        dsp.setPlotStyle(ps);
        p.addPlot(dsp);
      }
      p.plot();
    }
  }
}

private def plotBenchTPPP(b: Benchmark, res: Map[String, ImplGroupedResult[String]], output: Path, show: Boolean): Unit = {
	val bench = b.asInstanceOf[BenchmarkWithSpace[ThroughputPingPongRequest]];
	val space = bench.space.asInstanceOf[ParameterSpacePB[ThroughputPingPongRequest]];
	val meanGroupedParamsTime = res.mapValues(_.mapParams(space.paramsFromCSV));
	val meanGroupedParams = ImplResults.mapData(
		meanGroupedParamsTime,
		(req: ThroughputPingPongRequest, meanTime: Double) => {
			val totalMessages = req.messagesPerPair * req.parallelism * 2; // 1 Ping  and 1 Pong per exchange
			val throughput = (totalMessages.toDouble*1000.0)/meanTime; // msgs/s (1000 to get from ms to s)
			throughput
		}
	);
	{
		val meanGroupedPipelining = ImplResults.slices(
			meanGroupedParams,
			(req: ThroughputPingPongRequest) => (req.messagesPerPair, req.pipelineSize, req.staticOnly),
			(req: ThroughputPingPongRequest) => req.parallelism
		);
		meanGroupedPipelining.foreach { case (params, impls) =>
			val p = new JavaPlot();
			if (!show) {
				val outfile = output / s"${b.symbol}-msgs${params._1}-pipe${params._2}-static${params._3}.eps";
				val epsf = new terminal.PostscriptTerminal(outfile.toString);
				epsf.setColor(true);
		        p.setTerminal(epsf);
			}
			p.getAxis("x").setLabel("parallelism (#pairs)");
			p.getAxis("y").setLabel("throughput (msgs/s)");
			p.setTitle(s"${b.name} (#msgs/pair = ${params._1}, #pipeline size = ${params._2}, static = ${params._3})");
			impls.foreach { case (key, res) =>
				val dsp = new gplot.DataSetPlot(res);
				dsp.setTitle(res.implLabel);
				val ps = new style.PlotStyle(style.Style.LINESPOINTS);
				val colour = colourMap(key);
				ps.setLineType(colour);
				ps.setPointType(pointMap(key));
				dsp.setPlotStyle(ps);
				p.addPlot(dsp);
			}
			p.plot();
		}
	}
	{
		val meanGroupedPipelining = ImplResults.slices(
			meanGroupedParams,
			(req: ThroughputPingPongRequest) => (req.parallelism, req.pipelineSize, req.staticOnly),
			(req: ThroughputPingPongRequest) => req.messagesPerPair
		);
		meanGroupedPipelining.foreach { case (params, impls) =>
			val p = new JavaPlot();
			if (!show) {
				val outfile = output / s"${b.symbol}-pairs${params._1}-pipe${params._2}-static${params._3}.eps";
				val epsf = new terminal.PostscriptTerminal(outfile.toString);
				epsf.setColor(true);
		        p.setTerminal(epsf);
			}
			p.getAxis("x").setLabel("#msgs/pair");
			p.getAxis("y").setLabel("throughput (msgs/s)");
			p.setTitle(s"${b.name} (#pairs = ${params._1}, #pipeline size = ${params._2}, static = ${params._3})");
			impls.foreach { case (key, res) =>
				val dsp = new gplot.DataSetPlot(res);
				dsp.setTitle(res.implLabel);
				val ps = new style.PlotStyle(style.Style.LINESPOINTS);
				val colour = colourMap(key);
				ps.setLineType(colour);
				ps.setPointType(pointMap(key));
				dsp.setPlotStyle(ps);
				p.addPlot(dsp);
			}
			p.plot();
		}
	}
	{
		val staticImpls = ImplResults.paramsToImpl(
			meanGroupedParams,
			(label: String, req: ThroughputPingPongRequest) => s"${label} (${if (req.staticOnly) { "Static" } else { "Alloc." }})"
		);
		val meanGrouped = ImplResults.slices(
			staticImpls,
			(req: ThroughputPingPongRequest) => (req.messagesPerPair, req.parallelism),
			(req: ThroughputPingPongRequest) => req.pipelineSize
		);
		meanGrouped.foreach { case (params, impls) =>
			val merged = ImplResults.merge(impls);
			//println("Merged:\n" + merged);
			val p = new JavaPlot();
			//GNUPlot.getDebugger().setLevel(gutils.Debug.VERBOSE);
			if (!show) {
				val outfile = output / s"${b.symbol}-msgs${params._1}-pairs${params._2}.eps";
				val epsf = new terminal.PostscriptTerminal(outfile.toString);
				epsf.setColor(true);
		        p.setTerminal(epsf);
			}
			p.getAxis("x").setLabel("pipeline size (#msgs)");
			p.getAxis("y").setLabel("throughput (msgs/s)");
			p.setTitle(s"${b.name} (#msgs/pair = ${params._1}, #pairs = ${params._2})");
			p.set("boxwidth", "0.8");
			p.set("style", "data histograms");
			merged.labels.zipWithIndex.foreach { case (label, i) =>
				//println(s"Setting $label at $i (column ${i + merged.dataColumnOffset})");
				val dsp = new gplot.DataSetPlot(merged);
				if (i == 0) {
					dsp.set("using", s"""${i + merged.dataColumnOffset + 1}:xtic(${1})""");
					// val ps = new style.PlotStyle(style.Style.HISTOGRAMS);
					// dsp.setPlotStyle(ps);
				} else {
					dsp.set("using", s"""${i + merged.dataColumnOffset + 1}""");
				}
				dsp.setTitle(label);
				//val ps = new style.PlotStyle();
				val ps = new style.PlotStyle(style.Style.HISTOGRAMS);
				val colour = colourMap2.find {
					case (k, v) => label.toLowerCase().startsWith(k.toLowerCase())
				}.map(_._2).getOrElse(style.NamedPlotColor.BLACK);
				ps.setLineType(colour);
				// // ps.setPointType(pointMap(key));
				dsp.setPlotStyle(ps);
				p.addPlot(dsp);
				val definitionSB = new java.lang.StringBuilder();
				dsp.retrieveDefinition(definitionSB);
				val definition = definitionSB.toString();
				//println(s"Definition: $definition");
			}
			p.plot();
		}
	}
}

// From here: http://javaplot.panayotis.com/doc/com/panayotis/gnuplot/style/NamedPlotColor.html
private val colourMap: Map[String, style.PlotColor] = Map(
	"AKKA" -> style.NamedPlotColor.SKYBLUE,
	"AKKATYPED" -> style.NamedPlotColor.ROYALBLUE,
	"KOMPACTAC" -> style.NamedPlotColor.SEA_GREEN,
	"KOMPACTCO" -> style.NamedPlotColor.SPRING_GREEN,
	"KOMPACTMIX" -> style.NamedPlotColor.FOREST_GREEN,
	"KOMPICSJ" -> style.NamedPlotColor.VIOLET,
	"KOMPICSSC" -> style.NamedPlotColor.DARK_RED,
	"KOMPICSSC2" -> style.NamedPlotColor.RED,
	"ACTIX" -> style.NamedPlotColor.DARK_BLUE,
	"ERLANG" -> style.NamedPlotColor.NAVY,
	"RIKER" -> style.NamedPlotColor.MIDNIGHT_BLUE
);

private val colourMap2: Map[String, style.PlotColor] = Map(
	"Akka" -> style.NamedPlotColor.SKYBLUE,
	"Akka Typed" -> style.NamedPlotColor.ROYALBLUE,
	"Kompact Actor" -> style.NamedPlotColor.SEA_GREEN,
	"Kompact Component" -> style.NamedPlotColor.SPRING_GREEN,
	"Kompact Mixed" -> style.NamedPlotColor.TURQUOISE ,
	"Kompics Java" -> style.NamedPlotColor.VIOLET,
	"Kompics Scala 1.x" -> style.NamedPlotColor.DARK_RED,
	"Kompics Scala 2.x" -> style.NamedPlotColor.RED,
	"Actix" -> style.NamedPlotColor.DARK_BLUE,
	"Erlang" -> style.NamedPlotColor.NAVY,
	"Riker" -> style.NamedPlotColor.MIDNIGHT_BLUE
);

private val pointMap: Map[String, Int] = 
	List("AKKA", "AKKATYPED", "KOMPACTAC", "KOMPACTCO", "KOMPACTMIX", "KOMPICSJ", "KOMPICSSC", "KOMPICSSC2", "ACTIX", "ERLANG", "RIKER").zipWithIndex.toMap.mapValues(_ + 1);
