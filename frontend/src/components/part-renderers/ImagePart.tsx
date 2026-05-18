import React from 'react';
import { Part } from '../../types/part';

export const ImagePart: React.FC<{ part: Extract<Part, { type: 'image' }> }> = ({ part }) => (
  <div className="my-2">
    <img src={part.url} alt={part.alt || 'image'} className="max-w-full rounded" />
  </div>
);
