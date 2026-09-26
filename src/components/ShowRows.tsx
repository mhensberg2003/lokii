import type { ShowRow } from "../lib/catalog";
import { Row } from "./ui/Row";
import { PosterCard, PosterCardSkeleton } from "./ui/PosterCard";

export function ShowRowView({ row }: { row: ShowRow }) {
  if (row.shows.length === 0) return null;
  return (
    <Row title={row.title}>
      {row.shows.map((show) => (
        <PosterCard key={show.id} show={show} />
      ))}
    </Row>
  );
}

export function SkeletonRows({ count = 3 }: { count?: number }) {
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        <Row key={i} title=" ">
          {Array.from({ length: 8 }, (_, j) => (
            <PosterCardSkeleton key={j} />
          ))}
        </Row>
      ))}
    </>
  );
}
